// Shortwave - device_indicator.rs
// Copyright (C) 2024-2025  Felix Häcker <haeckerfelix@gnome.org>
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

use std::marker::PhantomData;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{subclass, Properties};
use gtk::{glib, CompositeTemplate};

use crate::app::SwApplication;
use crate::audio::SwPlayer;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/device_indicator.ui")]
    #[properties(wrapper_type = super::SwDeviceIndicator)]
    pub struct SwDeviceIndicator {
        #[template_child]
        button: TemplateChild<gtk::Button>,
        #[template_child]
        device_label: TemplateChild<gtk::Label>,

        #[property(get=Self::player)]
        pub player: PhantomData<SwPlayer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwDeviceIndicator {
        const NAME: &'static str = "SwDeviceIndicator";
        type ParentType = adw::Bin;
        type Type = super::SwDeviceIndicator;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
            klass.set_css_name("device-indicator");
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwDeviceIndicator {}

    impl WidgetImpl for SwDeviceIndicator {}

    impl BinImpl for SwDeviceIndicator {}

    #[gtk::template_callbacks]
    impl SwDeviceIndicator {
        fn player(&self) -> SwPlayer {
            SwApplication::default().player()
        }

        #[template_callback]
        async fn disconnect(&self) {
            let obj = self.obj();

            obj.set_sensitive(false);
            obj.player().disconnect_device().await;
            obj.set_sensitive(true);
        }
    }
}

glib::wrapper! {
    pub struct SwDeviceIndicator(ObjectSubclass<imp::SwDeviceIndicator>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl SwDeviceIndicator {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for SwDeviceIndicator {
    fn default() -> Self {
        Self::new()
    }
}
