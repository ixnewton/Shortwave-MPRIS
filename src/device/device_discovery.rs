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
use std::collections::HashMap;
use std::time::Duration;

use adw::prelude::*;
use async_std::future;
use async_std::stream::StreamExt;
use futures_util::pin_mut;
use glib::subclass::prelude::*;
use glib::{clone, Properties};
use gtk::glib;
use mdns::Error;

use super::{SwDevice, SwDeviceKind, SwDeviceModel};
use crate::i18n::i18n;

mod imp {
    use super::*;

    const CAST_SERVICE: &str = "_googlecast._tcp.local";

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
            let stream = mdns::discover::all(CAST_SERVICE, Duration::from_secs(3))?.listen();
            pin_mut!(stream);

            while let Some(Ok(response)) = stream.next().await {
                if let Some(addr) = response.ip_addr() {
                    let txt_records: Vec<&str> = response.txt_records().collect();
                    let mut values = HashMap::new();

                    for value in txt_records {
                        let parts: Vec<&str> = value.splitn(2, '=').collect();
                        if parts.len() == 2 {
                            values.insert(parts[0].to_string(), parts[1].to_string());
                        }
                    }

                    let device = SwDevice::new(
                        values.get("id").unwrap_or(&addr.to_string()),
                        SwDeviceKind::Cast,
                        values.get("fn").unwrap_or(&i18n("Google Cast Device")),
                        values.get("md").unwrap_or(&i18n("Unknown Model")),
                        &addr.to_string(),
                    );
                    self.devices.add_device(&device);
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
        let _ = future::timeout(Duration::from_secs(15), self.imp().discover_cast_devices()).await;

        debug!("Device scan ended!");
        self.imp().is_scanning.set(false);
        self.notify_is_scanning();
    }
}

impl Default for SwDeviceDiscovery {
    fn default() -> Self {
        Self::new()
    }
}
