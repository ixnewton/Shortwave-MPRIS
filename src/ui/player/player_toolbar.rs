// Shortwave - player_toolbar.rs
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
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/player_toolbar.ui")]
    #[properties(wrapper_type = super::SwPlayerToolbar)]
    pub struct SwPlayerToolbar {
        #[property(get)]
        pub player: SwPlayer,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwPlayerToolbar {
        const NAME: &'static str = "SwPlayerToolbar";
        type ParentType = adw::Bin;
        type Type = super::SwPlayerToolbar;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwPlayerToolbar {}

    impl WidgetImpl for SwPlayerToolbar {}

    impl BinImpl for SwPlayerToolbar {}
}

glib::wrapper! {
    pub struct SwPlayerToolbar(ObjectSubclass<imp::SwPlayerToolbar>)
        @extends gtk::Widget, adw::Bin;
}

#[gtk::template_callbacks]
impl SwPlayerToolbar {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for SwPlayerToolbar {
    fn default() -> Self {
        Self::new()
    }
}
