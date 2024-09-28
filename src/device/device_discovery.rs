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
use std::time::Duration;

use adw::prelude::*;
use async_io::Timer;
use futures_util::future::select;
use glib::subclass::prelude::*;
use glib::{clone, Properties};
use gtk::glib;
use mdns_sd::{Error, ServiceDaemon, ServiceEvent};

use super::{SwDevice, SwDeviceKind, SwDeviceModel};
use crate::i18n::i18n;

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
        let discover = self.imp().discover_cast_devices();
        let timeout = Timer::after(Duration::from_secs(15));
        let _ = select(pin!(discover), pin!(timeout)).await;

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
