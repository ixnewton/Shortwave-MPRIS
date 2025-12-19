// Shortwave - dlna_sender.rs
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

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use adw::prelude::*;
use glib::clone;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib;
use log::{debug, error, info, warn};
use reqwest::blocking::Client;
use tiny_http::{Method, Response, Server, StatusCode};
use url::Url;

// Helper function to send SOAP actions to DLNA devices
fn soap_action(control_url: &str, service_type: &str, action: &str, body: &str) -> Result<String, Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    
    let soap_envelope = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
<s:Body>
<u:{} xmlns:u="{}">
{}
</u:{}>
</s:Body>
</s:Envelope>"#,
        action, service_type, body, action
    );
    
    let response = client
        .post(control_url)
        .header("Content-Type", "text/xml; charset=utf-8")
        .header("SOAPAction", format!("\"{}#{}\"", service_type, action))
        .body(soap_envelope)
        .send()?;
    
    if response.status().is_success() {
        Ok(response.text()?)
    } else {
        Err(format!("SOAP action failed: {}", response.status()).into())
    }
}

// Helper function to extract value from SOAP response
fn extract_soap_value(response: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);
    
    if let Some(start) = response.find(&start_tag) {
        if let Some(end) = response.find(&end_tag) {
            return Some(response[start + start_tag.len()..end].trim().to_string());
        }
    }
    None
}

// Helper function to fetch device description and extract service URLs
fn fetch_device_services(device_url: &str) -> Result<(String, String), Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    
    let response = client.get(device_url).send()?;
    let xml_content = response.text()?;
    
    debug!("DLNA: Device description XML: {}", xml_content);
    
    // Extract service control URLs by searching entire XML
    let mut av_transport_url = None;
    let mut rendering_control_url = None;
    
    // Find AVTransport service anywhere in XML (handle line breaks)
    if let Some(service_start) = xml_content.find("urn:schemas-upnp-org:service:AVTransport:1") {
        debug!("DLNA: Found AVTransport serviceType in XML");
        
        // Search backwards from serviceType to find <service> start
        let service_block_start = xml_content[0..service_start].rfind("<service>")
            .unwrap_or(0);
        
        // Search forwards to find </service> end
        let service_block_end = xml_content[service_start..].find("</service>")
            .map(|pos| service_start + pos + 9)
            .unwrap_or(xml_content.len());
        
        let service_block = &xml_content[service_block_start..service_block_end];
        debug!("DLNA: AVTransport service block: {}", service_block);
        
        // Extract controlURL (handle whitespace and line breaks)
        if let Some(url_start) = service_block.find("<controlURL>") {
            if let Some(url_end) = service_block.find("</controlURL>") {
                let url = &service_block[url_start + 13..url_end];
                let url = url.trim(); // Remove whitespace
                let base_url = Url::parse(device_url)?;
                let full_url = base_url.join(url)?;
                av_transport_url = Some(full_url.to_string());
                debug!("DLNA: Found AVTransport service at: {}", full_url);
            }
        }
    }
    
    // Find RenderingControl service anywhere in XML (handle line breaks)
    if let Some(service_start) = xml_content.find("urn:schemas-upnp-org:service:RenderingControl:1") {
        debug!("DLNA: Found RenderingControl serviceType in XML");
        
        // Search backwards from serviceType to find <service> start
        let service_block_start = xml_content[0..service_start].rfind("<service>")
            .unwrap_or(0);
        
        // Search forwards to find </service> end
        let service_block_end = xml_content[service_start..].find("</service>")
            .map(|pos| service_start + pos + 9)
            .unwrap_or(xml_content.len());
        
        let service_block = &xml_content[service_block_start..service_block_end];
        debug!("DLNA: RenderingControl service block: {}", service_block);
        
        // Extract controlURL (handle whitespace and line breaks)
        if let Some(url_start) = service_block.find("<controlURL>") {
            if let Some(url_end) = service_block.find("</controlURL>") {
                let url = &service_block[url_start + 13..url_end];
                let url = url.trim(); // Remove whitespace
                let base_url = Url::parse(device_url)?;
                let full_url = base_url.join(url)?;
                rendering_control_url = Some(full_url.to_string());
                debug!("DLNA: Found RenderingControl service at: {}", full_url);
            }
        }
    }
    
    match (av_transport_url, rendering_control_url) {
        (Some(av), Some(rc)) => Ok((av, rc)),
        (Some(av), None) => {
            warn!("DLNA: RenderingControl service not found, using only AVTransport");
            Ok((av, String::new()))
        }
        _ => {
            error!("DLNA: Required services not found in device description");
            error!("DLNA: Available services in XML: {:?}", 
                xml_content.matches("serviceType>").count());
            Err("Required services not found".into())
        }
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwDlnaSender)]
    pub struct SwDlnaSender {
        #[property(get)]
        pub stream_url: RefCell<String>,
        #[property(get)]
        pub cover_url: RefCell<String>,
        #[property(get)]
        pub title: RefCell<String>,
        #[property(get, set, type = f64)]
        pub volume: Cell<f64>,
        #[property(get)]
        pub is_connected: Cell<bool>,

        pub device: RefCell<Option<String>>,  // Store device URL instead of Device object
        pub av_transport_url: RefCell<Option<String>>,  // Store AVTransport control URL
        pub rendering_control_url: RefCell<Option<String>>,  // Store RenderingControl control URL
        
        // Proxy server components
        pub proxy_thread: RefCell<Option<JoinHandle<()>>>,
        pub proxy_shutdown: Arc<AtomicBool>,
        pub proxy_port: Cell<u16>,
        pub local_ip: RefCell<String>,
        pub original_stream_url: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwDlnaSender {
        const NAME: &'static str = "SwDlnaSender";
        type Type = super::SwDlnaSender;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwDlnaSender {}

    impl SwDlnaSender {
        pub fn set_volume(&self, volume: f64) {
            self.volume.set(volume);

            if self.obj().is_connected() {
                glib::spawn_future_local(clone!(
                    #[weak(rename_to = this)]
                    self,
                    #[strong]
                    volume,
                    async move {
                        if let Err(e) = this.set_volume_internal(volume).await {
                            warn!("Failed to set DLNA volume: {}", e);
                        }
                    }
                ));
            }
        }

        async fn set_volume_internal(&self, volume: f64) -> Result<(), Box<dyn Error>> {
            if let Some(ref rc_url) = *self.rendering_control_url.borrow() {
                let volume_percent = (volume * 100.0) as u32;
                let body = format!(
                    "<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredVolume>{}</DesiredVolume>",
                    volume_percent
                );
                soap_action(rc_url, "urn:schemas-upnp-org:service:RenderingControl:1", "SetVolume", &body)?;
            }

            Ok(())
        }
    }
}

glib::wrapper! {
    pub struct SwDlnaSender(ObjectSubclass<imp::SwDlnaSender>);
}

impl SwDlnaSender {
    pub fn new() -> Self {
        glib::Object::new()
    }

    // Start HTTP proxy server for streaming
    fn start_proxy_server(&self) -> Result<(), Box<dyn Error>> {
        let imp = self.imp();
        
        // Find an available port
        let port = 8080u16; // Could make this configurable
        imp.proxy_port.set(port);
        
        // Extract local IP from device URL
        let device_url = imp.device.borrow().as_ref().unwrap().clone();
        let parsed_url = Url::parse(&device_url)?;
        let local_ip = parsed_url.host_str().unwrap_or("127.0.0.1").to_string();
        imp.local_ip.borrow_mut().clone_from(&local_ip);
        
        // Setup shutdown signal
        let shutdown = imp.proxy_shutdown.clone();
        shutdown.store(false, Ordering::SeqCst);
        
        // Store original stream URL for proxy
        let original_url = imp.stream_url.borrow().clone();
        imp.original_stream_url.borrow_mut().clone_from(&original_url);
        
        // Start proxy server in background thread
        let shutdown_clone = shutdown.clone();
        let stream_url_clone = original_url.clone();
        
        let thread = thread::spawn(move || {
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            
            match Server::http(addr) {
                Ok(server) => {
                    info!("DLNA: Proxy server started on port {}", port);
                    
                    for request in server.incoming_requests() {
                        // Check for shutdown signal
                        if shutdown_clone.load(Ordering::SeqCst) {
                            break;
                        }
                        
                        // Handle stream requests
                        if request.url() == "/stream" {
                            match Self::handle_stream_request(&request, &stream_url_clone) {
                                Ok(response) => {
                                    let _ = request.respond(response);
                                }
                                Err(e) => {
                                    error!("DLNA: Proxy request failed: {}", e);
                                    let _ = request.respond(Response::from_string(
                                        format!("Proxy error: {}", e)
                                    ).with_status_code(StatusCode(500)));
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("DLNA: Failed to start proxy server: {}", e);
                }
            }
            
            info!("DLNA: Proxy server stopped");
        });
        
        imp.proxy_thread.borrow_mut().replace(thread);
        Ok(())
    }

    // Stop HTTP proxy server
    fn stop_proxy_server(&self) {
        let imp = self.imp();
        
        // Signal shutdown
        imp.proxy_shutdown.store(true, Ordering::SeqCst);
        
        // Join thread
        if let Some(thread) = imp.proxy_thread.borrow_mut().take() {
            let _ = thread.join();
        }
        
        info!("DLNA: Proxy server stopped");
    }

    // Handle proxy stream requests
    fn handle_stream_request(request: &tiny_http::Request, stream_url: &str) -> Result<Response<std::io::Cursor<Vec<u8>>>, Box<dyn Error>> {
        if request.method() != &Method::Get {
            return Err("Only GET requests supported".into());
        }

        // Fetch the original stream
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        
        let response = client.get(stream_url).send()?;
        
        // Get the content as bytes
        let content = response.bytes()?;
        
        // Create proxy response with proper headers
        let mut proxy_response = Response::from_data(content);
        
        // Set content type for audio streams
        proxy_response = proxy_response.with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"audio/mpeg"[..]).unwrap()
        );
        
        Ok(proxy_response)
    }

    pub fn connect(&self, address: &str) -> Result<(), Box<dyn Error>> {
        if self.is_connected() {
            self.disconnect();
        }

        // Use the device URL directly (from discovery)
        let device_url = if address.starts_with("http") {
            address.to_string()
        } else {
            format!("http://{}", address)
        };

        // Fetch device description and extract service URLs
        let (av_transport_url, rendering_control_url) = fetch_device_services(&device_url)?;
        
        // Store the URLs
        *self.imp().device.borrow_mut() = Some(device_url.clone());
        *self.imp().av_transport_url.borrow_mut() = Some(av_transport_url);
        *self.imp().rendering_control_url.borrow_mut() = Some(rendering_control_url);
        
        self.imp().is_connected.set(true);
        self.notify_is_connected();
        
        // Get current volume from device
        if let Some(ref rc_url) = *self.imp().rendering_control_url.borrow() {
            let body = "<InstanceID>0</InstanceID><Channel>Master</Channel>";
            if let Ok(response) = soap_action(rc_url, "urn:schemas-upnp-org:service:RenderingControl:1", "GetVolume", body) {
                if let Some(volume_str) = extract_soap_value(&response, "CurrentVolume") {
                    if let Ok(volume) = volume_str.parse::<f64>() {
                        let normalized_volume = volume / 100.0;
                        self.imp().volume.set(normalized_volume);
                        self.notify_volume();
                    }
                }
            }
        }

        Ok(())
    }

    pub fn disconnect(&self) {
        if !self.is_connected() {
            return;
        }

        // Stop proxy server
        self.stop_proxy_server();

        *self.imp().device.borrow_mut() = None;
        *self.imp().av_transport_url.borrow_mut() = None;
        *self.imp().rendering_control_url.borrow_mut() = None;

        self.imp().is_connected.set(false);
        self.notify_is_connected();
    }

    pub fn load_media(&self, stream_url: &str, cover_url: &str, title: &str) -> Result<(), Box<dyn Error>> {
        *self.imp().stream_url.borrow_mut() = stream_url.to_string();
        *self.imp().cover_url.borrow_mut() = cover_url.to_string();
        *self.imp().title.borrow_mut() = title.to_string();

        self.notify_stream_url();
        self.notify_cover_url();
        self.notify_title();

        // Start proxy server for external stream
        if stream_url.starts_with("http") && !stream_url.contains("192.168.2.101") {
            self.start_proxy_server()?;
            
            // Use proxy URL instead of external URL
            let imp = self.imp();
            let local_ip = imp.local_ip.borrow();
            let port = imp.proxy_port.get();
            let proxy_url = format!("http://{}:{}/stream", local_ip, port);
            
            if let Some(ref av_url) = *imp.av_transport_url.borrow() {
                let metadata = format!(
                    r#"<DIDL-Lite xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/">
<item id="0" parentID="-1" restricted="0">
<dc:title>{}</dc:title>
<upnp:class>object.item.audioItem.musicTrack</upnp:class>
<res protocolInfo="http-get:*:audio/mpeg:*">{}</res>
</item>
</DIDL-Lite>"#,
                    title, proxy_url
                );

                let body = format!(
                    "<InstanceID>0</InstanceID><CurrentURI>{}</CurrentURI><CurrentURIMetaData>{}</CurrentURIMetaData>",
                    proxy_url,
                    metadata
                );

                soap_action(av_url, "urn:schemas-upnp-org:service:AVTransport:1", "SetAVTransportURI", &body)?;
            }
        } else {
            // Use original URL for local streams
            if let Some(ref av_url) = *self.imp().av_transport_url.borrow() {
                let metadata = format!(
                    r#"<DIDL-Lite xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/">
<item id="0" parentID="-1" restricted="0">
<dc:title>{}</dc:title>
<upnp:class>object.item.audioItem.musicTrack</upnp:class>
<res protocolInfo="http-get:*:audio/mpeg:*">{}</res>
</item>
</DIDL-Lite>"#,
                    title, stream_url
                );

                let body = format!(
                    "<InstanceID>0</InstanceID><CurrentURI>{}</CurrentURI><CurrentURIMetaData>{}</CurrentURIMetaData>",
                    stream_url,
                    metadata
                );

                soap_action(av_url, "urn:schemas-upnp-org:service:AVTransport:1", "SetAVTransportURI", &body)?;
            }
        }

        Ok(())
    }

    pub fn start_playback(&self) -> Result<(), Box<dyn Error>> {
        if !self.is_connected() {
            return Ok(());
        }

        if let Some(ref av_url) = *self.imp().av_transport_url.borrow() {
            let body = "<InstanceID>0</InstanceID><Speed>1</Speed>";
            soap_action(av_url, "urn:schemas-upnp-org:service:AVTransport:1", "Play", body)?;
        }

        Ok(())
    }

    pub fn stop_playback(&self) -> Result<(), Box<dyn Error>> {
        if !self.is_connected() {
            return Ok(());
        }

        if let Some(ref av_url) = *self.imp().av_transport_url.borrow() {
            let body = "<InstanceID>0</InstanceID>";
            soap_action(av_url, "urn:schemas-upnp-org:service:AVTransport:1", "Stop", body)?;
        }

        Ok(())
    }

    pub fn set_volume_dlna(&self, volume: f64) -> Result<(), Box<dyn Error>> {
        self.imp().volume.set(volume);
        self.notify_volume();

        if let Some(ref rc_url) = *self.imp().rendering_control_url.borrow() {
            let volume_percent = (volume * 100.0) as u32;
            let body = format!(
                "<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredVolume>{}</DesiredVolume>",
                volume_percent
            );
            soap_action(rc_url, "urn:schemas-upnp-org:service:RenderingControl:1", "SetVolume", &body)?;
        }

        Ok(())
    }

    pub fn set_volume_public(&self, volume: f64) {
        self.imp().set_volume(volume);
    }
}

impl Default for SwDlnaSender {
    fn default() -> Self {
        Self::new()
    }
}
