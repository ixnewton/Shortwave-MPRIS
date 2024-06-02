// Shortwave - gcast_discoverer.rs
// Copyright (C) 2021-2024  Felix Häcker <haeckerfelix@gnome.org>
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

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use async_channel::{Receiver, Sender};
use futures_util::pin_mut;
use futures_util::stream::StreamExt;
use gtk::glib::clone;
use mdns::{Error, Record, RecordKind};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GCastDevice {
    pub id: String,
    pub ip: IpAddr,
    pub name: String,
}

pub enum GCastDiscovererMessage {
    DiscoverStarted,
    DiscoverEnded,
    FoundDevice(GCastDevice),
}

#[derive(Debug)]
pub struct GCastDiscoverer {
    sender: Sender<GCastDiscovererMessage>,
    known_devices: Arc<Mutex<Vec<GCastDevice>>>,
}

impl GCastDiscoverer {
    pub fn new() -> (Self, Receiver<GCastDiscovererMessage>) {
        let (sender, receiver) = async_channel::bounded(10);
        let known_devices = Arc::new(Mutex::new(Vec::new()));

        let gcd = Self {
            sender,
            known_devices,
        };
        (gcd, receiver)
    }

    // TODO: Refactor this, and get rid of the message passing
    pub async fn discover(&self) -> Result<(), Error> {
        let known_devices = self.known_devices.clone();
        let sender = self.sender.clone();

        // Reset previous found devices
        known_devices.lock().unwrap().clear();

        debug!("Start discovering for google cast devices...");
        gtk::glib::source::idle_add_once(clone!(@strong sender => move || {
            crate::utils::send(&sender, GCastDiscovererMessage::DiscoverStarted);
        }));

        let stream =
            mdns::discover::all("_googlecast._tcp.local", std::time::Duration::from_secs(15))?
                .listen();
        pin_mut!(stream);

        while let Some(Ok(response)) = stream.next().await {
            let known_devices = known_devices.clone();
            let sender = sender.clone();
            if let Some(device) = Self::device(response) {
                if !known_devices.lock().unwrap().contains(&device) {
                    debug!("Found new google cast device!");
                    debug!("{:?}", device);
                    known_devices.lock().unwrap().insert(0, device.clone());
                    gtk::glib::source::idle_add_once(clone!(@strong sender => move || {
                        crate::utils::send(&sender, GCastDiscovererMessage::FoundDevice(device));
                    }));
                }
            }
        }

        gtk::glib::source::idle_add_once(clone!(@strong sender => move || {
            crate::utils::send(&sender, GCastDiscovererMessage::DiscoverEnded);
        }));
        debug!("GCast discovery ended.");

        Ok(())
    }

    pub fn device_by_ip_addr(&self, ip: IpAddr) -> Option<GCastDevice> {
        let devices = self.known_devices.lock().unwrap().to_vec();
        for device in devices.iter() {
            if device.ip == ip {
                return Some(device.clone());
            }
        }
        None
    }

    fn device(response: mdns::Response) -> Option<GCastDevice> {
        let mut values: HashMap<String, String> = HashMap::new();

        let addr = response
            .records()
            .filter_map(Self::record_to_ip_addr)
            .next();
        if addr.is_none() {
            debug!("Cast device does not advertise address.");
            return None;
        }

        // To get the values, we need to iterate the additional records and check the
        // TXT kind
        for record in response.additional {
            // Check if record kind is TXT
            if let mdns::RecordKind::TXT(v) = record.kind {
                // Iterate TXT values
                for value in v {
                    let tmp = value.split('=').collect::<Vec<&str>>();
                    values.insert(tmp[0].to_string(), tmp[1].to_string());
                }
                break;
            }
        }

        let device = GCastDevice {
            id: values.get("id").unwrap_or(&String::new()).to_string(),
            ip: addr.unwrap(),
            name: values.get("fn").unwrap_or(&String::new()).to_string(),
        };

        Some(device)
    }

    fn record_to_ip_addr(record: &Record) -> Option<IpAddr> {
        match record.kind {
            RecordKind::A(addr) => Some(addr.into()),
            RecordKind::AAAA(addr) => Some(addr.into()),
            _ => None,
        }
    }
}
