// Shortwave - device.rs
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

use std::cell::{Cell, OnceCell};

use adw::prelude::*;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib;

use super::SwDeviceKind;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwDevice)]
    pub struct SwDevice {
        #[property(get, set, construct_only)]
        id: OnceCell<String>,
        #[property(get, set, construct_only, builder(SwDeviceKind::default()))]
        kind: Cell<SwDeviceKind>,
        #[property(get, set, construct_only)]
        name: OnceCell<String>,
        #[property(get, set, construct_only)]
        model: OnceCell<String>,
        #[property(get, set, construct_only)]
        address: OnceCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwDevice {
        const NAME: &'static str = "SwDevice";
        type Type = super::SwDevice;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwDevice {}
}

glib::wrapper! {
    pub struct SwDevice(ObjectSubclass<imp::SwDevice>);
}

impl SwDevice {
    pub fn new(id: &str, kind: SwDeviceKind, name: &str, model: &str, address: &str) -> Self {
        glib::Object::builder()
            .property("id", id)
            .property("kind", kind)
            .property("name", name)
            .property("model", model)
            .property("address", address)
            .build()
    }
}
