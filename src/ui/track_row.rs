// Shortwave - track_row.rs
// Copyright (C) 2021-2025  Felix Häcker <haeckerfelix@gnome.org>
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

use std::cell::OnceCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{subclass, Properties};
use gtk::{glib, CompositeTemplate};

use crate::audio::SwRecordingState;
use crate::audio::SwTrack;
use crate::utils;

mod imp {
    use crate::app::SwApplication;

    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::SwTrackRow)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/track_row.ui")]
    pub struct SwTrackRow {
        #[template_child]
        pub save_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub saved_checkmark_button: TemplateChild<gtk::Button>,

        #[property(get, set, construct_only)]
        pub track: OnceCell<SwTrack>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwTrackRow {
        const NAME: &'static str = "SwTrackRow";
        type ParentType = adw::ActionRow;
        type Type = super::SwTrackRow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwTrackRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            let track = self.obj().track();
            track.insert_actions(&*obj);

            track
                .bind_property("title", &*self.obj(), "title")
                .sync_create()
                .build();

            track
                .bind_property("title", &*self.obj(), "tooltip-text")
                .sync_create()
                .build();

            track
                .bind_property("state", &*self.obj(), "subtitle")
                .transform_to(|b, state: SwRecordingState| {
                    let track = b.source().unwrap().downcast::<SwTrack>().unwrap();
                    let title = state.title();

                    let string = if state == SwRecordingState::Recorded {
                        utils::format_duration(track.duration(), true)
                    } else {
                        title
                    };
                    Some(string)
                })
                .sync_create()
                .build();

            track
                .bind_property("state", &*self.save_button, "visible")
                .transform_to(|b: &glib::Binding, state: SwRecordingState| {
                    let track = b.source().unwrap().downcast::<SwTrack>().unwrap();
                    Some(state == SwRecordingState::Recorded && !track.is_saved())
                })
                .sync_create()
                .build();

            track
                .bind_property("is-saved", &*self.save_button, "visible")
                .transform_to(|b: &glib::Binding, is_saved: bool| {
                    let track = b.source().unwrap().downcast::<SwTrack>().unwrap();
                    Some(track.state() == SwRecordingState::Recorded && !is_saved)
                })
                .sync_create()
                .build();

            track
                .bind_property("is-saved", &*self.saved_checkmark_button, "visible")
                .sync_create()
                .build();
        }
    }

    impl WidgetImpl for SwTrackRow {}

    impl ListBoxRowImpl for SwTrackRow {}

    impl PreferencesRowImpl for SwTrackRow {}

    impl ActionRowImpl for SwTrackRow {
        fn activate(&self) {
            SwApplication::default().show_track_dialog(&self.obj().track());
        }
    }
}

glib::wrapper! {
    pub struct SwTrackRow(ObjectSubclass<imp::SwTrackRow>)
        @extends gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow, adw::ActionRow;
}

impl SwTrackRow {
    pub fn new(track: SwTrack) -> Self {
        glib::Object::builder().property("track", &track).build()
    }
}
