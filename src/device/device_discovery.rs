// Shortwave - device_discovery.rs
// Copyright (C) 2024  Felix Häcker <haeckerfelix@gnome.org>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::cell::Cell;
use std::pin::pin;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use adw::prelude::*;
use async_io::Timer;
use futures_util::future::{select, Either};
use glib::subclass::prelude::*;
use glib::{clone, Properties};
use gtk::glib;
use mdns_sd::{Error, ServiceDaemon, ServiceEvent};
use tokio::sync::oneshot;

use super::{SwDevice, SwDeviceKind, SwDeviceModel};
use crate::i18n::i18n;

fn parse_ssdp_response(response: &str) -> Option<(String, String, String, String)> {
    debug!("DLNA: Parsing SSDP response...");
    
    let mut location = None;
    let mut host = None;
    
    // Parse HTTP headers to get LOCATION
    for line in response.lines() {
        if line.starts_with("LOCATION:") {
            location = Some(line[9..].trim().to_string());
            debug!("DLNA: Found LOCATION: {}", location.as_ref().unwrap());
            break;
        }
    }
    
    // Extract host from location URL
    if let Some(ref loc) = location {
        if let Ok(url) = url::Url::parse(loc) {
            host = Some(url.host_str().unwrap_or("unknown").to_string());
            debug!("DLNA: Extracted host: {}", host.as_ref().unwrap());
        }
    }
    
    let location = location?;
    let host = host.unwrap_or_else(|| "unknown".to_string());
    
    // Fetch device description XML to get proper friendlyName and device type
    let (friendly_name, device_type) = fetch_device_info(&location).unwrap_or_else(|_| {
        // Fallback to a generic name with IP if fetch fails
        (format!("DLNA Device ({})", host), "unknown".to_string())
    });
    
    debug!("DLNA: Parsed device - Location: {}, Name: {}, Type: {}, Host: {}", location, friendly_name, device_type, host);
    
    Some((location, friendly_name, device_type, host))
}

fn fetch_device_info(location: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    debug!("DLNA: Fetching device description from {}", location);
    
    // Use blocking HTTP client in the background thread
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()?;
    
    let response = client.get(location).send()?;
    let xml_content = response.text()?;
    
    debug!("DLNA: Got device description XML ({} bytes)", xml_content.len());
    
    // Parse XML to extract friendlyName
    let friendly_name = if let Some(start) = xml_content.find("<friendlyName>") {
        if let Some(end) = xml_content.find("</friendlyName>") {
            let name = xml_content[start + 13..end].trim().to_string();
            debug!("DLNA: Extracted friendlyName: {}", name);
            name
        } else {
            "Unknown Device".to_string()
        }
    } else {
        "Unknown Device".to_string()
    };
    
    // Parse XML to extract deviceType
    let device_type = if let Some(start) = xml_content.find("<deviceType>") {
        if let Some(end) = xml_content.find("</deviceType>") {
            let dev_type = xml_content[start + 12..end].trim().to_string();
            debug!("DLNA: Extracted deviceType: {}", dev_type);
            dev_type
        } else {
            "unknown".to_string()
        }
    } else {
        "unknown".to_string()
    };
    
    Ok((friendly_name, device_type))
}

mod imp {
    use super::*;

    const CAST_SERVICE: &str = "_googlecast._tcp.local.";

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwDeviceDiscovery)]
    pub struct SwDeviceDiscovery {
        #[property(get)]
        devices: SwDeviceModel,
        #[property(get)]
        pub is_scanning: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwDeviceDiscovery {
        const NAME: &'static str = "SwDeviceDiscovery";
        type Type = super::SwDeviceDiscovery;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwDeviceDiscovery {
        fn constructed(&self) {
            self.parent_constructed();

            glib::spawn_future_local(clone!(
                #[weak(rename_to = imp)]
                self,
                async move {
                    imp.obj().scan().await;
                }
            ));
        }
    }

    impl SwDeviceDiscovery {
        pub async fn discover_cast_devices(&self) -> Result<(), Error> {
            let mdns = ServiceDaemon::new()?;
            let receiver = mdns.browse(CAST_SERVICE)?;

            while let Ok(event) = receiver.recv_async().await {
                if let ServiceEvent::ServiceResolved(info) = event {
                    let host = info.get_addresses().iter().next().unwrap().to_string();

                    let device = SwDevice::new(
                        info.get_property("id")
                            .map(|txt| txt.val_str())
                            .unwrap_or(&host),
                        SwDeviceKind::Cast,
                        info.get_property("fn")
                            .map(|txt| txt.val_str())
                            .unwrap_or(&i18n("Google Cast Device")),
                        info.get_property("md")
                            .map(|txt| txt.val_str())
                            .unwrap_or(&i18n("Unknown Model")),
                        &host,
                    );
                    self.devices.add_device(&device);
                }
            }

            Ok(())
        }

        pub async fn discover_dlna_devices(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            debug!("Starting DLNA device discovery using raw SSDP with pa-dlna improvements...");
            
            // Use tokio oneshot channel for truly async communication
            let (sender, receiver) = oneshot::channel::<Result<Vec<(String, String, String, String)>, String>>();
            
            std::thread::spawn(move || {
                debug!("DLNA discovery thread started");
                
                let result = std::thread::spawn(move || {
                    // Raw SSDP implementation with pa-dlna improvements
                    debug!("DLNA: Creating SSDP M-SEARCH request...");
                    
                    // Create UDP socket for multicast
                    let socket = match UdpSocket::bind("0.0.0.0:0") {
                        Ok(socket) => {
                            debug!("DLNA: UDP socket created successfully");
                            socket
                        }
                        Err(e) => {
                            error!("DLNA: Failed to create UDP socket: {}", e);
                            return Err(format!("Socket creation failed: {}", e));
                        }
                    };
                    
                    socket.set_read_timeout(Some(Duration::from_secs(5))).ok();
                    
                    // SSDP M-SEARCH message for root devices (pa-dlna approach)
                    let search_msg = format!(
                        "M-SEARCH * HTTP/1.1\r\n\
                         HOST: 239.255.255.250:1900\r\n\
                         MAN: \"ssdp:discover\"\r\n\
                         ST: upnp:rootdevice\r\n\
                         MX: 2\r\n\r\n"
                    );
                    
                    debug!("DLNA: Using upnp:rootdevice search target (pa-dlna approach)");
                    
                    // Send to SSDP multicast address
                    let multicast_addr: SocketAddr = "239.255.255.250:1900".parse().unwrap();
                    
                    // Send multiple M-SEARCH requests like pa-dlna (3 requests with 0.2s intervals)
                    for i in 0..3 {
                        debug!("DLNA: Sending M-SEARCH request #{}", i + 1);
                        if let Err(e) = socket.send_to(search_msg.as_bytes(), multicast_addr) {
                            error!("DLNA: Failed to send M-SEARCH #{}: {}", i + 1, e);
                            return Err(format!("Send failed: {}", e));
                        }
                        
                        // Wait 0.2 seconds between requests (pa-dlna approach)
                        if i < 2 {
                            std::thread::sleep(Duration::from_millis(200));
                        }
                    }
                    
                    debug!("DLNA: All M-SEARCH requests sent, waiting for responses...");
                    
                    let mut device_infos = Vec::new();
                    let mut buffer = [0u8; 4096];
                    let mut device_count = 0;
                    
                    // Listen for responses
                    loop {
                        match socket.recv_from(&mut buffer) {
                            Ok((bytes_read, src_addr)) => {
                                device_count += 1;
                                let response = String::from_utf8_lossy(&buffer[..bytes_read]);
                                debug!("DLNA: Received response #{} from {}", device_count, src_addr);
                                debug!("DLNA: Response preview: {}", &response[..response.len().min(200)]);
                                
                                // Parse SSDP response
                                if let Some(device_info) = parse_ssdp_response(&response) {
                                    debug!("DLNA: Parsed device - URL: {}, Name: {}", device_info.0, device_info.1);
                                    device_infos.push(device_info);
                                } else {
                                    debug!("DLNA: Failed to parse device response");
                                }
                            }
                            Err(e) => {
                                debug!("DLNA: Stopping listening: {}", e);
                                break;
                            }
                        }
                    }
                    
                    debug!("DLNA: Discovery completed, found {} valid devices", device_infos.len());
                    Ok(device_infos)
                }).join().unwrap_or_else(|_| Err("Thread panicked".to_string()));
                
                let _ = sender.send(result);
            });

            // Set up timeout to check for results
            let timeout = Timer::after(Duration::from_secs(12));
            
            match select(pin!(receiver), pin!(timeout)).await {
                Either::Left((Ok(Ok(device_infos)), _)) => {
                    debug!("DLNA: Discovery completed successfully");
                    // Add devices to glib model on main thread
                    for (url, name, device_type, host) in device_infos {
                        // Filter for only media renderer devices
                        if device_type.contains("MediaRenderer") {
                            // Extract device type name for model field
                            let device_type_name = device_type.split(':').nth(3).unwrap_or("MediaRenderer");
                            let device_name = name.trim_start_matches('>');
                            debug!("DLNA: Adding media renderer device: {} ({})", device_name, device_type);
                            let device = SwDevice::new(
                                &url,  // Use the full discovery URL as address
                                SwDeviceKind::Dlna,
                                device_name,  // Device name only
                                &format!("DLNA {}", device_type_name),  // Model as subtitle to match Cast styling
                                &url,  // Use the full discovery URL as address
                            );
                            self.devices.add_device(&device);
                        } else {
                            debug!("DLNA: Skipping non-renderer device: {} ({})", name, device_type);
                        }
                    }
                }
                Either::Left((Ok(Err(e)), _)) => {
                    error!("DLNA discovery failed: {}", e);
                    return Err(e.into());
                }
                Either::Left((Err(_), _)) => {
                    error!("DLNA discovery communication failed");
                    return Err("Communication failed".into());
                }
                Either::Right(_) => {
                    debug!("DLNA discovery timeout reached");
                    warn!("DLNA discovery timed out");
                }
            }

            Ok(())
        }
    }
}

glib::wrapper! {
    pub struct SwDeviceDiscovery(ObjectSubclass<imp::SwDeviceDiscovery>);
}

impl SwDeviceDiscovery {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub async fn scan(&self) {
        if self.is_scanning() {
            debug!("Device scan is already active");
            return;
        }

        debug!("Start device scan...");
        self.imp().is_scanning.set(true);
        self.notify_is_scanning();

        self.devices().clear();
        
        // Run both Cast and DLNA discovery in parallel
        let cast_discovery = self.imp().discover_cast_devices();
        let dlna_discovery = self.imp().discover_dlna_devices();
        let timeout = Timer::after(Duration::from_secs(15));
        
        match select(pin!(cast_discovery), pin!(select(pin!(dlna_discovery), pin!(timeout)))).await {
            Either::Left((cast_result, _)) => {
                if let Err(e) = cast_result {
                    warn!("Cast discovery failed: {}", e);
                }
            }
            Either::Right((Either::Left((dlna_result, _)), _)) => {
                if let Err(e) = dlna_result {
                    warn!("DLNA discovery failed: {}", e);
                    debug!("DLNA discovery error details: {:?}", e);
                }
            }
            Either::Right((Either::Right(_), _)) => {
                debug!("Device discovery timeout reached");
            }
        }

        debug!("Device scan ended!");
        self.imp().is_scanning.set(false);
        self.notify_is_scanning();
    }

    pub fn stop(&self) {
        if self.is_scanning() {
            debug!("Stopping device discovery scan...");
            self.imp().is_scanning.set(false);
            self.notify_is_scanning();
            self.devices().clear();
            debug!("Device discovery stopped and cleared");
        }
    }
}

impl Default for SwDeviceDiscovery {
    fn default() -> Self {
        Self::new()
    }
}
