// Shortwave - recording_indicator.rs
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

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass, Properties};
use gtk::{glib, CompositeTemplate};

use crate::audio::{SwRecordingState, SwTrack};
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/recording_indicator.ui")]
    #[properties(wrapper_type = super::SwRecordingIndicator)]
    pub struct SwRecordingIndicator {
        #[template_child]
        duration_label: TemplateChild<gtk::Label>,

        #[property(get, set=Self::set_track)]
        track: RefCell<Option<SwTrack>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwRecordingIndicator {
        const NAME: &'static str = "SwRecordingIndicator";
        type ParentType = gtk::Button;
        type Type = super::SwRecordingIndicator;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwRecordingIndicator {}

    impl WidgetImpl for SwRecordingIndicator {}

    impl ButtonImpl for SwRecordingIndicator {}

    impl SwRecordingIndicator {
        fn set_track(&self, track: Option<SwTrack>) {
            if let Some(track) = &track {
                track
                    .bind_property("duration", &*self.duration_label, "label")
                    .transform_to(|_, duration: u64| Some(utils::format_duration(duration, true)))
                    .sync_create()
                    .build();

                self.update_state(track.state());
                track.connect_state_notify(clone!(
                    #[weak(rename_to = imp)]
                    self,
                    move |track| {
                        imp.update_state(track.state());
                    }
                ));
            } else {
                self.duration_label
                    .set_text(&utils::format_duration(0, true));
                self.update_state(SwRecordingState::IdleDisabled);
            }

            *self.track.borrow_mut() = track;
        }

        fn update_state(&self, state: SwRecordingState) {
            if state == SwRecordingState::Recording {
                self.obj().add_css_class("active");
            } else {
                self.obj().remove_css_class("active");
            }

            self.obj().set_tooltip_text(Some(&state.title()));
        }
    }
}

glib::wrapper! {
    pub struct SwRecordingIndicator(ObjectSubclass<imp::SwRecordingIndicator>)
        @extends gtk::Widget, gtk::Button;
}

impl SwRecordingIndicator {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for SwRecordingIndicator {
    fn default() -> Self {
        Self::new()
    }
}
