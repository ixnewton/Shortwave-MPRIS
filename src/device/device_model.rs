// Shortwave - device_model.rs
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

use std::cell::RefCell;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use indexmap::map::IndexMap;

use super::SwDevice;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct SwDeviceModel {
        pub map: RefCell<IndexMap<String, SwDevice>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwDeviceModel {
        const NAME: &'static str = "SwDeviceModel";
        type Type = super::SwDeviceModel;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for SwDeviceModel {}

    impl ListModelImpl for SwDeviceModel {
        fn item_type(&self) -> glib::Type {
            SwDevice::static_type()
        }

        fn n_items(&self) -> u32 {
            self.map.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            self.map
                .borrow()
                .get_index(position.try_into().unwrap())
                .map(|(_, o)| o.clone().upcast::<glib::Object>())
        }
    }
}

glib::wrapper! {
    pub struct SwDeviceModel(ObjectSubclass<imp::SwDeviceModel>) @implements gio::ListModel;
}

impl SwDeviceModel {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub(super) fn add_device(&self, device: &SwDevice) {
        let pos = {
            let mut map = self.imp().map.borrow_mut();
            if map.contains_key(&device.id()) {
                return;
            }

            map.insert(device.id(), device.clone());
            (map.len() - 1) as u32
        };

        self.items_changed(pos, 0, 1);
    }

    pub(super) fn clear(&self) {
        let len = self.n_items();
        self.imp().map.borrow_mut().clear();
        self.items_changed(0, len, 0);
    }
}

impl Default for SwDeviceModel {
    fn default() -> Self {
        Self::new()
    }
}
