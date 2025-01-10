// Shortwave - track_dialog.rs
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

use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{subclass, Properties};
use gtk::{gio, glib, CompositeTemplate};

use super::{SwStationDialog, ToastWindow};
use crate::app::SwApplication;
use crate::audio::{SwRecordingMode, SwRecordingState, SwTrack};
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/track_dialog.ui")]
    #[properties(wrapper_type = super::SwTrackDialog)]
    pub struct SwTrackDialog {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        saved_label: TemplateChild<gtk::Label>,
        #[template_child]
        subtitle_label: TemplateChild<gtk::Label>,
        #[template_child]
        duration_label: TemplateChild<gtk::Label>,
        #[template_child]
        description_label: TemplateChild<gtk::Label>,
        #[template_child]
        save_track_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        save_track_switch: TemplateChild<gtk::Switch>,
        #[template_child]
        cancel_button: TemplateChild<gtk::Button>,
        #[template_child]
        save_button: TemplateChild<gtk::Button>,
        #[template_child]
        play_button: TemplateChild<gtk::Button>,
        #[template_child]
        recording_label: TemplateChild<gtk::Label>,

        #[property(get, set, construct_only, type=SwTrack)]
        track: RefCell<Option<SwTrack>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwTrackDialog {
        const NAME: &'static str = "SwTrackDialog";
        type ParentType = adw::Dialog;
        type Type = super::SwTrackDialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwTrackDialog {
        fn constructed(&self) {
            self.parent_constructed();
            let player = SwApplication::default().player();

            let track = self.obj().track();
            track.insert_actions(&*self.obj());

            track
                .bind_property("state", &*self.subtitle_label, "label")
                .transform_to(|_, state: SwRecordingState| Some(state.title()))
                .sync_create()
                .build();

            track
                .bind_property("is-saved", &*self.saved_label, "visible")
                .sync_create()
                .build();

            track
                .bind_property("state", &*self.description_label, "label")
                .transform_to(|_, state: SwRecordingState| Some(state.description()))
                .sync_create()
                .build();

            track
                .bind_property("duration", &*self.duration_label, "label")
                .transform_to(|b, d: u64| {
                    let duration = utils::format_duration(d, false);
                    let track = b.source().unwrap().downcast::<SwTrack>().unwrap();
                    let file = track.file();

                    Some(
                        if let Ok(res) = file.measure_disk_usage(
                            gio::FileMeasureFlags::NONE,
                            gio::Cancellable::NONE,
                            None,
                        ) {
                            format!("{} - {}", &duration, &glib::format_size(res.0))
                        } else {
                            duration
                        },
                    )
                })
                .sync_create()
                .build();

            track
                .bind_property("state", &*self.duration_label, "visible")
                .transform_to(|_, state: SwRecordingState| {
                    Some(
                        state == SwRecordingState::Recording
                            || state == SwRecordingState::Recorded
                            || state == SwRecordingState::DiscardedBelowThreshold,
                    )
                })
                .sync_create()
                .build();

            track
                .bind_property("save-when-recorded", &*self.save_track_switch, "active")
                .sync_create()
                .bidirectional()
                .build();

            track
                .bind_property("state", &*self.save_track_row, "sensitive")
                .transform_to(|_, state: SwRecordingState| {
                    Some(state == SwRecordingState::Recording)
                })
                .sync_create()
                .build();

            player
                .bind_property("recording-mode", &*self.save_track_row, "visible")
                .transform_to(|_, state: SwRecordingMode| Some(state == SwRecordingMode::Decide))
                .sync_create()
                .build();

            track
                .bind_property("state", &*self.cancel_button, "visible")
                .transform_to(|_, state: SwRecordingState| {
                    Some(state == SwRecordingState::Recording)
                })
                .sync_create()
                .build();

            track
                .bind_property("state", &*self.save_button, "visible")
                .transform_to(|_, state: SwRecordingState| {
                    Some(state != SwRecordingState::Recording)
                })
                .sync_create()
                .build();

            track
                .bind_property("is-saved", &*self.save_button, "visible")
                .transform_to(|b, is_saved: bool| {
                    let track = b.source().unwrap().downcast::<SwTrack>().unwrap();
                    Some(!is_saved && track.state() != SwRecordingState::Recording)
                })
                .sync_create()
                .build();

            track
                .bind_property("is-saved", &*self.play_button, "visible")
                .sync_create()
                .build();

            self.recording_label.connect_activate_link(|label, _| {
                label
                    .root()
                    .unwrap()
                    .activate_action("win.show-preferences", None)
                    .unwrap();
                glib::Propagation::Stop
            });
        }
    }

    impl WidgetImpl for SwTrackDialog {}

    impl AdwDialogImpl for SwTrackDialog {}

    #[gtk::template_callbacks]
    impl SwTrackDialog {
        #[template_callback]
        fn show_station_details(&self) {
            let dialog = SwStationDialog::new(&self.obj().track().station());
            dialog.present(Some(&*self.obj()));
        }
    }
}

glib::wrapper! {
    pub struct SwTrackDialog(ObjectSubclass<imp::SwTrackDialog>)
        @extends gtk::Widget, adw::Dialog;
}

impl SwTrackDialog {
    pub fn new(track: &SwTrack) -> Self {
        glib::Object::builder().property("track", track).build()
    }
}

impl ToastWindow for SwTrackDialog {
    fn toast_overlay(&self) -> adw::ToastOverlay {
        self.imp().toast_overlay.clone()
    }
}
