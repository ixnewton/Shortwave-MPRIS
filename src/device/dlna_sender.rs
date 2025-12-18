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
use std::time::Duration;

use adw::prelude::*;
use glib::clone;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib;
use reqwest;
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
    
    // Extract service control URLs
    let mut av_transport_url = None;
    let mut rendering_control_url = None;
    
    // Find AVTransport service
    if let Some(start) = xml_content.find("<service>") {
        let services_section = &xml_content[start..];
        for service in services_section.split("<service>") {
            if service.contains("urn:upnp-org:serviceId:AVTransport") {
                if let Some(url_start) = service.find("<controlURL>") {
                    if let Some(url_end) = service.find("</controlURL>") {
                        let url = &service[url_start + 13..url_end];
                        let base_url = Url::parse(device_url)?;
                        let full_url = base_url.join(url.trim())?;
                        av_transport_url = Some(full_url.to_string());
                    }
                }
            }
            
            if service.contains("urn:upnp-org:serviceId:RenderingControl") {
                if let Some(url_start) = service.find("<controlURL>") {
                    if let Some(url_end) = service.find("</controlURL>") {
                        let url = &service[url_start + 13..url_end];
                        let base_url = Url::parse(device_url)?;
                        let full_url = base_url.join(url.trim())?;
                        rendering_control_url = Some(full_url.to_string());
                    }
                }
            }
        }
    }
    
    match (av_transport_url, rendering_control_url) {
        (Some(av), Some(rc)) => Ok((av, rc)),
        _ => Err("Required services not found".into()),
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
