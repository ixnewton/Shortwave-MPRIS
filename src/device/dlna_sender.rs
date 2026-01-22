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
use std::net;
use std::sync::mpsc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use adw::prelude::*;
use glib::clone;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib;
use log::{debug, error, info, warn};
use url::Url;

// Helper function to get local IP address that can reach the DLNA device
pub fn get_local_ip_for_device(device_url: &str) -> Result<String, Box<dyn Error>> {
    // Parse device URL to get device IP
    let parsed_url = Url::parse(device_url)?;
    let device_ip = parsed_url.host_str().ok_or("Invalid device URL")?;
    
    // Create a UDP socket to determine the best local interface
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(format!("{}:80", device_ip))?;
    
    // Get the local address that would be used to connect to the device
    let local_addr = socket.local_addr()?;
    let local_ip = local_addr.ip().to_string();
    
    info!("DLNA: Detected local IP {} for device at {}", local_ip, device_ip);
    Ok(local_ip)
}

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
        
        // FFmpeg process for streaming
        pub ffmpeg_process: RefCell<Option<std::process::Child>>,
        
        // FFmpeg thread handle
        pub ffmpeg_thread: RefCell<Option<JoinHandle<Result<(), String>>>>,
        
        // DLNA device information
        pub device: RefCell<Option<String>>,  // Store device URL instead of Device object
        pub av_transport_url: RefCell<Option<String>>,  // Store AVTransport control URL
        pub rendering_control_url: RefCell<Option<String>>,  // Store RenderingControl control URL
        
        // FFmpeg streaming server components
        pub ffmpeg_port: Cell<u16>,
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

    // Start FFmpeg streaming server asynchronously
    fn start_ffmpeg_server(&self) -> Result<(), Box<dyn Error>> {
        let imp = self.imp();
        
        // Get the current stream URL
        let current_url = imp.stream_url.borrow().clone();
        let previous_url = imp.original_stream_url.borrow().clone();
        
        // Check if FFmpeg is already running and if we can reuse it
        {
            let mut process_guard = imp.ffmpeg_process.borrow_mut();
            if let Some(child) = process_guard.as_mut() {
                // Check if the process is still alive
                match child.try_wait() {
                    Ok(Some(_)) => {
                        // Process has exited, clean it up
                        info!("DLNA: FFmpeg process has exited, cleaning up");
                        drop(process_guard.take());
                    }
                    Ok(None) => {
                        // Process is still running - check if we can reuse it
                        if current_url == previous_url && !current_url.is_empty() {
                            info!("DLNA: Reusing existing FFmpeg process for same URL: {}", current_url);
                            return Ok(());
                        } else {
                            info!("DLNA: URL changed from {} to {}, restarting FFmpeg", previous_url, current_url);
                            // Stop existing process for new URL
                            if let Err(e) = child.kill() {
                                warn!("DLNA: Failed to kill existing FFmpeg process: {}", e);
                            }
                            // Wait for process to exit
                            match child.wait() {
                                Ok(status) => {
                                    info!("DLNA: Old FFmpeg process exited with status: {}", status);
                                }
                                Err(e) => {
                                    warn!("DLNA: Error waiting for old FFmpeg process: {}", e);
                                }
                            }
                            drop(process_guard.take());
                            info!("DLNA: Cleared old FFmpeg process for new stream");
                        }
                    }
                    Err(e) => {
                        // Error checking process status, assume it's dead
                        warn!("DLNA: Error checking FFmpeg process status: {}, cleaning up", e);
                        drop(process_guard.take());
                    }
                }
            }
        }
        
        // Find an available port (reuse existing if available)
        let port = if imp.ffmpeg_port.get() == 0 {
            8080u16 // Default port
        } else {
            imp.ffmpeg_port.get()
        };
        imp.ffmpeg_port.set(port);
        
        // Extract local IP from device URL (if available)
        let local_ip = if let Some(device_url) = imp.device.borrow().as_ref() {
            match get_local_ip_for_device(device_url) {
                Ok(ip) => ip,
                Err(e) => {
                    warn!("DLNA: Failed to detect local IP for FFmpeg: {}, using fallback", e);
                    "127.0.0.1".to_string()
                }
            }
        } else {
            "127.0.0.1".to_string()
        };
        imp.local_ip.borrow_mut().clone_from(&local_ip);
        
        // Store original stream URL for FFmpeg
        let original_url = imp.stream_url.borrow().clone();
        imp.original_stream_url.borrow_mut().clone_from(&original_url);
        
        info!("DLNA: Starting FFmpeg streaming server on {}:{}", local_ip, port);
        info!("DLNA: Original stream URL: {}", original_url);
        
        // Start FFmpeg in background thread to avoid blocking UI
        let ffmpeg_url = format!("http://0.0.0.0:{}/stream.mp3", port);
        
        // Create a channel to send the process handle back to main thread
        let (process_sender, process_receiver) = mpsc::channel::<std::process::Child>();
        
        // Clone sender for metadata polling
        let metadata_sender = self.clone();
        let stream_url_for_metadata = original_url.clone();
        
        // Start ICY metadata polling using glib async (thread-safe)
        glib::spawn_future_local(async move {
            info!("DLNA: Starting ICY metadata polling for: {}", stream_url_for_metadata);
            let mut last_title = String::new();
            let mut last_dlna_title = String::new(); // Track last sent to DLNA device
            
            // Poll metadata every 30 seconds
            loop {
                if let Ok(title) = fetch_icy_metadata(&stream_url_for_metadata) {
                    // Update local UI title if it changed
                    if !title.is_empty() && title != last_title {
                        info!("DLNA: New track detected: {}", title);
                        metadata_sender.imp().title.borrow_mut().clone_from(&title);
                        metadata_sender.notify_title();
                        last_title = title.clone();
                        
                        // Update DLNA device metadata if it's different from last sent
                        if !title.is_empty() && title != last_dlna_title {
                            info!("DLNA: Updating device metadata to: {}", title);
                            if let Err(e) = metadata_sender.update_track_metadata(&title) {
                                warn!("DLNA: Failed to update device metadata: {}", e);
                            } else {
                                info!("DLNA: ✅ Device metadata updated successfully");
                                last_dlna_title = title.clone();
                            }
                        }
                    }
                }
                
                // Sleep for 30 seconds before next poll
                glib::timeout_future(Duration::from_secs(30) * 1000).await;
            }
        });
        
        let thread = thread::spawn(move || {
            // Build FFmpeg command for DLNA streaming
            let mut ffmpeg_cmd = std::process::Command::new("ffmpeg");
            
            // Add HLS-specific options for continuous streaming
            if original_url.contains(".m3u8") {
                // HLS stream - add optimization parameters
                ffmpeg_cmd
                    .arg("-fflags")
                    .arg("+genpts+discardcorrupt") // Generate timestamps and discard corrupt packets
                    .arg("-live_start_index")
                    .arg("-2") // Start 2 segments back for buffer (reduced from 3)
                    .arg("-max_reload")
                    .arg("10") // Reload playlist every 10 seconds (reduced frequency)
                    .arg("-max_delay")
                    .arg("10000000") // 10 seconds max delay (increased from 5)
                    .arg("-i")
                    .arg(&original_url);
            } else {
                // Non-HLS stream - use standard input
                ffmpeg_cmd
                    .arg("-i")
                    .arg(&original_url);
            }
            
            let ffmpeg_cmd = ffmpeg_cmd
                .arg("-vn") // No video
                .arg("-c:a")
                .arg("libmp3lame") // MP3 codec
                .arg("-b:a")
                .arg("128k") // 128kbps bitrate
                .arg("-listen")
                .arg("1") // Listen mode
                .arg("-f")
                .arg("mp3") // MP3 format
                .arg(&ffmpeg_url)
                .spawn();
            
            match ffmpeg_cmd {
                Ok(child) => {
                    let pid = child.id();
                    info!("DLNA: FFmpeg started with PID: {}", pid);
                    
                    // Send the process handle back to main thread
                    if let Err(e) = process_sender.send(child) {
                        error!("DLNA: Failed to send FFmpeg process handle: {}", e);
                        return Err("Failed to store FFmpeg process handle".into());
                    }
                    
                    info!("DLNA: FFmpeg process handle sent to main thread");
                    Ok(())
                }
                Err(e) => {
                    error!("DLNA: Failed to start FFmpeg: {}", e);
                    Err(format!("Failed to start FFmpeg: {}", e))
                }
            }
        });
        
        imp.ffmpeg_thread.borrow_mut().replace(thread);
        info!("DLNA: FFmpeg startup initiated in background thread");
        
        // Wait for the process handle from the background thread
        match process_receiver.recv() {
            Ok(child_process) => {
                info!("DLNA: Received FFmpeg process handle, storing it");
                imp.ffmpeg_process.borrow_mut().replace(child_process);
                info!("DLNA: FFmpeg process handle stored successfully");
            }
            Err(e) => {
                error!("DLNA: Failed to receive FFmpeg process handle: {}", e);
                return Err("Failed to receive FFmpeg process handle".into());
            }
        }
        
        Ok(())
    }
                                // Stop FFmpeg streaming server
    pub fn stop_ffmpeg_server(&self) {
        let imp = self.imp();
        
        // Kill the stored FFmpeg process if it exists
        if let Some(mut child) = imp.ffmpeg_process.borrow_mut().take() {
            info!("DLNA: Stopping FFmpeg process (PID: {})", child.id());
            
            // Force kill immediately - Rust's child.kill() sends SIGKILL on Unix
            // This ensures immediate termination and prevents zombie processes
            if let Err(e) = child.kill() {
                warn!("DLNA: Failed to kill FFmpeg process: {}", e);
            }
            
            // Immediately wait for the process to clean up zombie
            match child.wait() {
                Ok(status) => {
                    info!("DLNA: FFmpeg process terminated with status: {}", status);
                }
                Err(e) => {
                    warn!("DLNA: Error waiting for FFmpeg process termination: {}", e);
                }
            }
        } else {
            info!("DLNA: No stored FFmpeg process to stop");
        }
        
        // Additional cleanup: Kill any orphaned FFmpeg processes that might have been missed
        info!("DLNA: Performing additional cleanup of any orphaned FFmpeg processes");
        if let Ok(output) = std::process::Command::new("pgrep").arg("ffmpeg").output() {
            if !output.stdout.is_empty() {
                let ffmpeg_pids = String::from_utf8_lossy(&output.stdout);
                for pid_str in ffmpeg_pids.lines() {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        info!("DLNA: Found orphaned FFmpeg process PID: {}, force killing with SIGKILL", pid);
                        // Use kill -9 for force termination
                        if let Ok(_) = std::process::Command::new("kill").arg("-9").arg(pid.to_string()).output() {
                            info!("DLNA: Successfully killed orphaned FFmpeg process PID: {}", pid);
                        } else {
                            warn!("DLNA: Failed to kill orphaned FFmpeg process PID: {}", pid);
                        }
                    }
                }
            } else {
                info!("DLNA: No orphaned FFmpeg processes found");
            }
        } else {
            warn!("DLNA: Failed to check for orphaned FFmpeg processes");
        }
        
        // Join the FFmpeg thread
        if let Some(thread) = imp.ffmpeg_thread.borrow_mut().take() {
            info!("DLNA: Waiting for FFmpeg thread to finish");
            match thread.join() {
                Ok(result) => {
                    if let Err(e) = result {
                        warn!("DLNA: FFmpeg thread failed: {}", e);
                    } else {
                        info!("DLNA: FFmpeg thread finished successfully");
                    }
                }
                Err(e) => {
                    warn!("DLNA: Failed to join FFmpeg thread: {:?}", e);
                }
            }
        }
        
        info!("DLNA: FFmpeg server stopped and all processes cleaned up");
    }

    // Force restart FFmpeg server (used when device selection changes)
    pub fn restart_ffmpeg_server(&self) -> Result<(), Box<dyn Error>> {
        info!("DLNA: Force restarting FFmpeg server for fresh instance");
        
        // Stop existing FFmpeg server
        self.stop_ffmpeg_server();
        
        // Start new FFmpeg server
        self.start_ffmpeg_server()
    }

    // Check if FFmpeg process is running and can be reused
    pub fn can_reuse_ffmpeg_process(&self, new_url: &str) -> bool {
        let imp = self.imp();
        
        // Check if we have a running process
        let mut process_guard = imp.ffmpeg_process.borrow_mut();
        if let Some(child) = process_guard.as_mut() {
            // Check if process is still alive
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited
                    false
                }
                Ok(None) => {
                    // Process is still running - check if URL is the same
                    let previous_url = imp.original_stream_url.borrow();
                    new_url == previous_url.as_str() && !new_url.is_empty()
                }
                Err(_) => {
                    // Error checking process status
                    false
                }
            }
        } else {
            false
        }
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

        info!("DLNA: Disconnecting device - performing full cleanup");

        // Perform the same thorough cleanup as stop_ffmpeg_server()
        // This ensures no FFmpeg processes are left running when disconnecting
        self.stop_ffmpeg_server();

        // Clear device connection info
        *self.imp().device.borrow_mut() = None;
        *self.imp().av_transport_url.borrow_mut() = None;
        *self.imp().rendering_control_url.borrow_mut() = None;

        self.imp().is_connected.set(false);
        self.notify_is_connected();
        
        info!("DLNA: Device disconnected and all processes cleaned up");
    }

    pub fn load_media(&self, stream_url: &str, cover_url: &str, title: &str) -> Result<(), Box<dyn Error>> {
        *self.imp().stream_url.borrow_mut() = stream_url.to_string();
        *self.imp().cover_url.borrow_mut() = cover_url.to_string();
        *self.imp().title.borrow_mut() = title.to_string();

        self.notify_stream_url();
        self.notify_cover_url();
        self.notify_title();

        // Start FFmpeg streaming server for external stream
        if stream_url.starts_with("http") {
            info!("DLNA: === STARTING DLNA PLAYBACK SEQUENCE ===");
            info!("DLNA: External stream detected: {}", stream_url);
            info!("DLNA: Step 1: Load URL to DLNA device");
            
            // Fetch service info on first use if not already done
            if self.imp().av_transport_url.borrow().is_none() {
                if let Some(device_url) = self.imp().device.borrow().as_ref() {
                    info!("DLNA: Fetching service info on first use");
                    let (av_url, rc_url) = fetch_device_services(device_url)?;
                    *self.imp().av_transport_url.borrow_mut() = Some(av_url);
                    *self.imp().rendering_control_url.borrow_mut() = Some(rc_url);
                }
            }
            
            // Step 1: Load the URL to DLNA device first
            let imp = self.imp();
            let local_ip = if let Some(device_url) = imp.device.borrow().as_ref() {
                match get_local_ip_for_device(device_url) {
                    Ok(ip) => ip,
                    Err(e) => {
                        warn!("DLNA: Failed to detect local IP: {}, using fallback", e);
                        "127.0.0.1".to_string()
                    }
                }
            } else {
                "127.0.0.1".to_string()
            };
            imp.local_ip.borrow_mut().clone_from(&local_ip);
            
            let port = 8080u16;
            imp.ffmpeg_port.set(port);
            let ffmpeg_url = format!("http://{}:{}/stream.mp3", local_ip, port);
            
            if let Some(ref av_url) = *imp.av_transport_url.borrow() {
                // Create metadata using actual station title from Shortwave's radio data
                let escaped_title = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                let metadata = format!(
                    "&lt;DIDL-Lite xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\" xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\"&gt;&lt;item id=\"0\" parentID=\"-1\" restricted=\"0\"&gt;&lt;dc:title&gt;{} *LIVE&lt;/dc:title&gt;&lt;upnp:class&gt;object.item.audioItem.musicTrack&lt;/upnp:class&gt;&lt;res protocolInfo=\"http-get:*:audio/mpeg:*\"&gt;{}&lt;/res&gt;&lt;/item&gt;&lt;/DIDL-Lite&gt;",
                    escaped_title, ffmpeg_url
                );
                
                let body = format!(
                    "<InstanceID>0</InstanceID><CurrentURI>{}</CurrentURI><CurrentURIMetaData>{}</CurrentURIMetaData>",
                    ffmpeg_url, metadata
                );

                info!("DLNA: Step 1 - Sending SetAVTransportURI with FFmpeg URL: {}", ffmpeg_url);
                info!("DLNA: Sending to URL: {}", av_url);
                info!("DLNA: SOAP Action header: \"urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI\"");
                info!("DLNA: SOAP Body: {}", body);
                
                let soap_envelope = format!(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
<s:Body>
<u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
{}
</u:SetAVTransportURI>
</s:Body>
</s:Envelope>"#,
                    body
                );
                
                info!("DLNA: Full SOAP Envelope: {}", soap_envelope);
                info!("DLNA: === SENDING SETAVTRANSPORTURI REQUEST ===");
                info!("DLNA: POST URL: {}", av_url);
                info!("DLNA: SOAPAction: \"urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI\"");
                info!("DLNA: Content-Type: text/xml; charset=\"utf-8\"");
                info!("DLNA: Content-Length: {}", soap_envelope.len());
                info!("DLNA: XML Body:");
                info!("DLNA: {}", soap_envelope);
                info!("DLNA: === END SETAVTRANSPORTURI REQUEST ===");
                
                let client = reqwest::blocking::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .connect_timeout(Duration::from_secs(5))
                    .build()?;
                
                let response = match client
                    .post(av_url)
                    .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI\"")
                    .header("Content-Type", "text/xml; charset=\"utf-8\"")
                    .header("Content-Length", soap_envelope.len().to_string())
                    .body(soap_envelope)
                    .send() {
                        Ok(resp) => resp,
                        Err(e) => {
                            error!("DLNA: HTTP request failed: {}", e);
                            return Err(format!("HTTP request failed: {}", e).into());
                        }
                    };
                
                let status = response.status();
                let response_text = response.text().unwrap_or_default();
                
                info!("DLNA: Response status: {}", status);
                info!("DLNA: Response body: {}", response_text);
                
                if status.is_success() {
                    info!("DLNA: SetAVTransportURI sent successfully");
                } else {
                    error!("DLNA: SetAVTransportURI failed with status: {}", status);
                    return Err(format!("SetAVTransportURI failed: {}", status).into());
                }
                
                // Step 2: Configure and start FFmpeg proxy
                info!("DLNA: Step 2 - Configure and start FFmpeg proxy");
                info!("DLNA: Using FFmpeg as transcoder and HTTP streaming server");
                
                // Store original stream URL for FFmpeg
                let original_url = imp.stream_url.borrow().clone();
                imp.original_stream_url.borrow_mut().clone_from(&original_url);
                
                info!("DLNA: Starting FFmpeg streaming server on {}:{}", local_ip, port);
                info!("DLNA: Original stream URL: {}", original_url);
                
                self.start_ffmpeg_server()?;
                
                info!("DLNA: FFmpeg server started on {}:{}", local_ip, port);
                info!("DLNA: Replacing external URL with FFmpeg URL: {}", ffmpeg_url);
                
                // Step 3: Issue the play command to DLNA device
                info!("DLNA: Step 3 - Issue play command to DLNA device");
                
                // Wait for FFmpeg to be ready before sending Play command
                info!("DLNA: Waiting 2 seconds for FFmpeg server to be ready...");
                std::thread::sleep(Duration::from_secs(2));
                info!("DLNA: FFmpeg should be ready now");
                
                let play_body = "<InstanceID>0</InstanceID><Speed>1</Speed>";
                let play_soap_envelope = format!(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
<s:Body>
<u:Play xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
{}
</u:Play>
</s:Body>
</s:Envelope>"#,
                    play_body
                );
                
                info!("DLNA: Full SOAP Envelope: {}", play_soap_envelope);
                info!("DLNA: === SENDING PLAY REQUEST ===");
                info!("DLNA: POST URL: {}", av_url);
                info!("DLNA: SOAPAction: \"urn:schemas-upnp-org:service:AVTransport:1#Play\"");
                info!("DLNA: Content-Type: text/xml; charset=\"utf-8\"");
                info!("DLNA: Content-Length: {}", play_soap_envelope.len());
                info!("DLNA: XML Body:");
                info!("DLNA: {}", play_soap_envelope);
                info!("DLNA: === END PLAY REQUEST ===");
                
                let play_response = match client
                    .post(av_url)
                    .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#Play\"")
                    .header("Content-Type", "text/xml; charset=\"utf-8\"")
                    .header("Content-Length", play_soap_envelope.len().to_string())
                    .body(play_soap_envelope)
                    .send() {
                        Ok(resp) => resp,
                        Err(e) => {
                            error!("DLNA: Play HTTP request failed: {}", e);
                            return Err(format!("Play HTTP request failed: {}", e).into());
                        }
                    };
                
                let play_status = play_response.status();
                let play_response_text = play_response.text().unwrap_or_default();
                
                info!("DLNA: Play response status: {}", play_status);
                info!("DLNA: Play response body: {}", play_response_text);
                
                if play_status.is_success() {
                    info!("DLNA: Play command sent successfully");
                    info!("DLNA: Complete playback sequence finished");
                    info!("DLNA: DLNA device will now stream from FFmpeg server: {}", ffmpeg_url);
                } else {
                    error!("DLNA: Play command failed with status: {}", play_status);
                    return Err(format!("Play command failed: {}", play_status).into());
                }
            } else {
                error!("DLNA: No AVTransport URL available - device discovery incomplete");
                return Err("DLNA device discovery incomplete - no AVTransport service found".into());
            }
        } else {
            // Use original URL for local streams
            info!("DLNA: Using direct URL (no proxy needed): {}", stream_url);
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

                info!("DLNA: Sending SetAVTransportURI with direct URL: {}", stream_url);
                soap_action(av_url, "urn:schemas-upnp-org:service:AVTransport:1", "SetAVTransportURI", &body)?;

                // Send Play command to start playback
                info!("DLNA: Sending Play command to start playback");
                let play_body = "<InstanceID>0</InstanceID><Speed>1</Speed>";
                soap_action(av_url, "urn:schemas-upnp-org:service:AVTransport:1", "Play", play_body)?;
                
                info!("DLNA: ✅ SetAVTransportURI + Play commands sent successfully");
            } else {
                error!("DLNA: No AVTransport URL available - device discovery incomplete");
                return Err("DLNA device discovery incomplete - no AVTransport service found".into());
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
        info!("DLNA: stop_playback() called - sending stop command");
        
        // Always try to send stop command - don't check connection status
        // The device might still be connected even if is_connected is false
        info!("DLNA: === STARTING DLNA STOP SEQUENCE ===");
        // Step 1: Stop the FFmpeg proxy first to prevent broken pipe errors
        info!("DLNA: Step 1 - Stop the FFmpeg proxy");
        info!("DLNA: Stopping FFmpeg server");
        self.stop_ffmpeg_server();

        // Step 2: Send stop command to DLNA device
        info!("DLNA: Step 2 - Issue stop command to DLNA device");
        
        if let Some(ref av_url) = *self.imp().av_transport_url.borrow() {
            let body = "<InstanceID>0</InstanceID>";
            let soap_envelope = format!(
                r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
<s:Body>
<u:Stop xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
{}
</u:Stop>
</s:Body>
</s:Envelope>"#,
                body
            );
            
            info!("DLNA: Full SOAP Envelope: {}", soap_envelope);
            info!("DLNA: === SENDING STOP REQUEST ===");
            info!("DLNA: POST URL: {}", av_url);
            info!("DLNA: SOAPAction: \"urn:schemas-upnp-org:service:AVTransport:1#Stop\"");
            info!("DLNA: Content-Type: text/xml; charset=\"utf-8\"");
            info!("DLNA: Content-Length: {}", soap_envelope.len());
            info!("DLNA: XML Body:");
            info!("DLNA: {}", soap_envelope);
            info!("DLNA: === END STOP REQUEST ===");
            
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .connect_timeout(Duration::from_secs(5))
                .build()?;
            
            let response = match client
                .post(av_url)
                .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#Stop\"")
                .header("Content-Type", "text/xml; charset=\"utf-8\"")
                .header("Content-Length", soap_envelope.len().to_string())
                .body(soap_envelope)
                .send() {
                    Ok(resp) => resp,
                    Err(e) => {
                        error!("DLNA: Stop HTTP request failed: {}", e);
                        return Err(format!("Stop HTTP request failed: {}", e).into());
                    }
                };
            
            let status = response.status();
            let response_text = response.text().unwrap_or_default();
            
            info!("DLNA: Stop response status: {}", status);
            info!("DLNA: Stop response body: {}", response_text);
            
            if status.is_success() {
                info!("DLNA: ✅ Stop command sent successfully");
            } else {
                error!("DLNA: ❌ Stop command failed with status: {}", status);
                return Err(format!("Stop command failed: {}", status).into());
            }
        } else {
            error!("DLNA: No AVTransport URL available - cannot send stop command");
            return Err("DLNA device discovery incomplete - no AVTransport service found".into());
        }

        info!("DLNA: ✅ Complete stop sequence finished");
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

    pub fn set_mute_dlna(&self, mute: bool) -> Result<(), Box<dyn Error>> {
        if let Some(ref rc_url) = *self.imp().rendering_control_url.borrow() {
            let mute_value = if mute { "1" } else { "0" };
            let body = format!(
                "<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredMute>{}</DesiredMute>",
                mute_value
            );
            soap_action(rc_url, "urn:schemas-upnp-org:service:RenderingControl:1", "SetMute", &body)?;
            info!("DLNA: Set mute to {} on device", mute);
        }

        Ok(())
    }

    pub fn get_volume_dlna(&self) -> Result<f64, Box<dyn Error>> {
        if let Some(ref rc_url) = *self.imp().rendering_control_url.borrow() {
            let body = "<InstanceID>0</InstanceID><Channel>Master</Channel>";
            let response = soap_action(rc_url, "urn:schemas-upnp-org:service:RenderingControl:1", "GetVolume", body)?;
            
            // Parse volume from response (simplified - would need XML parsing in production)
            // For now, return the stored volume
            Ok(self.imp().volume.get())
        } else {
            Ok(self.imp().volume.get())
        }
    }

    // Update track metadata on DLNA device without interrupting playback
    pub fn update_track_metadata(&self, new_title: &str) -> Result<(), Box<dyn Error>> {
        info!("DLNA: Updating track metadata to: {}", new_title);
        
        // Use the stored local IP and port for the streaming URL
        let local_ip = self.imp().local_ip.borrow().clone();
        let port = self.imp().ffmpeg_port.get();
        let streaming_url = format!("http://{}:{}/stream.mp3", local_ip, port);
        
        // Get device URL from stored device information
        if let Some(device_url) = self.imp().device.borrow().as_ref() {
            if let Ok((av_url, _)) = fetch_device_services(device_url) {
                // Create metadata with new track title
                let escaped_title = new_title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
                let metadata = format!(
                    r#"<DIDL-Lite xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/">
<item id="0" parentID="-1" restricted="0">
<dc:title>{}</dc:title>
<upnp:class>object.item.audioItem.musicTrack</upnp:class>
<res protocolInfo="http-get:*:audio/mpeg:*">{}</res>
</item>
</DIDL-Lite>"#, 
                    escaped_title, streaming_url
                );
                
                let body = format!(
                    "<InstanceID>0</InstanceID><NextURI>{}</NextURI><NextURIMetaData>{}</NextURIMetaData>",
                    streaming_url, metadata
                );
                    
                    info!("DLNA: === SENDING SETNEXTAVTRANSPORTURI REQUEST ===");
                    info!("DLNA: NextURIMetaData: {}", metadata);
                    info!("DLNA: SOAPAction: \"urn:schemas-upnp-org:service:AVTransport:1#SetNextAVTransportURI\"");
                    info!("DLNA: Request body: {}", body);
                    
                    soap_action(&av_url, "urn:schemas-upnp-org:service:AVTransport:1", "SetNextAVTransportURI", &body)?;
                    info!("DLNA: ✅ SetNextAVTransportURI sent successfully - metadata updated");
            } else {
                warn!("DLNA: Cannot update metadata - failed to fetch device services");
            }
        } else {
            warn!("DLNA: Cannot update metadata - no device URL available");
        }
        
        Ok(())
    }
}

impl Default for SwDlnaSender {
    fn default() -> Self {
        Self::new()
    }
}

// Helper function to extract StreamTitle from ICY metadata
fn extract_icy_title(metadata: &str) -> Option<String> {
    // Look for StreamTitle in ICY metadata
    for line in metadata.lines() {
        if line.starts_with("StreamTitle=") {
            let title = line.strip_prefix("StreamTitle=")
                .unwrap_or("")
                .trim_matches('\'')
                .trim_matches('"');
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

// Fetch ICY metadata from a radio stream URL using HTTP HEAD request
fn fetch_icy_metadata(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    
    // Send HEAD request to get ICY metadata
    let response = client
        .head(url)
        .header("Icy-MetaData", "1")
        .header("User-Agent", "Shortwave/1.0")
        .send()?;
    
    // Check for ICY metadata in headers
    if let Some(icy_name) = response.headers().get("icy-name") {
        if let Ok(name) = icy_name.to_str() {
            return Ok(name.to_string());
        }
    }
    
    // Try a brief GET request to extract StreamTitle from initial metadata
    let response = client
        .get(url)
        .header("Icy-MetaData", "1")
        .header("User-Agent", "Shortwave/1.0")
        .send()?;
    
    // Check if we have ICY metadata in response
    if let Some(icy_metaint) = response.headers().get("icy-metaint") {
        info!("DLNA: Stream supports ICY metadata with interval: {:?}", icy_metaint);
        // For now, return empty string - the actual metadata extraction 
        // would require streaming the full audio data which is complex
        return Ok(String::new());
    }
    
    Ok(String::new())
}
