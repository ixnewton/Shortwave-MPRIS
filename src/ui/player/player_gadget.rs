// Shortwave - player_gadget.rs
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
use crate::ui::SwVolumeControl;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/player_gadget.ui")]
    #[properties(wrapper_type = super::SwPlayerGadget)]
    pub struct SwPlayerGadget {
        #[template_child]
        volume_control: TemplateChild<SwVolumeControl>,

        #[property(get=Self::player)]
        pub player: PhantomData<SwPlayer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwPlayerGadget {
        const NAME: &'static str = "SwPlayerGadget";
        type ParentType = adw::Bin;
        type Type = super::SwPlayerGadget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwPlayerGadget {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj()
                .player()
                .bind_property("volume", &*self.volume_control, "volume")
                .sync_create()
                .bidirectional()
                .build();
        }
    }

    impl WidgetImpl for SwPlayerGadget {}

    impl BinImpl for SwPlayerGadget {}

    impl SwPlayerGadget {
        fn player(&self) -> SwPlayer {
            SwApplication::default().player()
        }
    }
}

glib::wrapper! {
    pub struct SwPlayerGadget(ObjectSubclass<imp::SwPlayerGadget>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

#[gtk::template_callbacks]
impl SwPlayerGadget {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for SwPlayerGadget {
    fn default() -> Self {
        Self::new()
    }
}
