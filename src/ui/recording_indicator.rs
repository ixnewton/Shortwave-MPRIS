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

use std::cell::Cell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass, Properties};
use gtk::{glib, CompositeTemplate};

use crate::audio::SwPlayer;
use crate::audio::{SwSong, SwSongState};
use crate::i18n::i18n;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/recording_indicator.ui")]
    #[properties(wrapper_type = super::SwRecordingIndicator)]
    pub struct SwRecordingIndicator {
        #[template_child]
        button: TemplateChild<gtk::MenuButton>,
        #[template_child]
        state_statuspage: TemplateChild<adw::StatusPage>,
        #[template_child]
        duration_label: TemplateChild<gtk::Label>,

        #[property(get)]
        player: SwPlayer,

        updating_duration: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwRecordingIndicator {
        const NAME: &'static str = "SwRecordingIndicator";
        type ParentType = adw::Bin;
        type Type = super::SwRecordingIndicator;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.set_css_name("recording-indicator");
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwRecordingIndicator {
        fn constructed(&self) {
            self.parent_constructed();

            self.set_song(self.player.song());
            self.player.connect_song_notify(clone!(
                #[weak(rename_to = imp)]
                self,
                move |player| {
                    imp.set_song(player.song());
                }
            ));
        }
    }

    impl WidgetImpl for SwRecordingIndicator {}

    impl BinImpl for SwRecordingIndicator {}

    impl SwRecordingIndicator {
        fn set_song(&self, song: Option<SwSong>) {
            self.duration_label.set_text(&Self::format_duration(0));
            self.updating_duration.set(false);

            if let Some(song) = song {
                self.update_state(song.state());
                song.connect_state_notify(clone!(
                    #[weak(rename_to = imp)]
                    self,
                    move |song| {
                        imp.update_state(song.state());
                    }
                ));
            }
        }

        fn update_state(&self, state: SwSongState) {
            if state == SwSongState::Recording {
                self.update_duration();
                self.obj().add_css_class("active");
            } else {
                self.obj().remove_css_class("active");
            }

            // title
            let title = match state {
                SwSongState::Recording => i18n("Recording in Progress"),
                SwSongState::Ignored => i18n("Ignored"),
                SwSongState::Incomplete => i18n("Incomplete"),
                _ => String::new(),
            };

            // description
            let description = match state {
                SwSongState::Recording => {
                    i18n("The current song will be recorded until a new song is detected.")
                }
                SwSongState::Ignored => {
                    i18n("No recording because the song title contains a word on the ignore list.")
                }
                SwSongState::Incomplete => i18n(
                    "The current song cannot be fully recorded. The beginning has been missed.",
                ),
                _ => String::new(),
            };

            self.state_statuspage.set_title(&title);
            self.state_statuspage.set_description(Some(&description));
        }

        fn update_duration(&self) {
            if self.updating_duration.get() {
                return;
            }
            self.updating_duration.set(true);

            glib::timeout_add_seconds_local(
                1,
                clone!(
                    #[weak(rename_to = imp)]
                    self,
                    #[upgrade_or_panic]
                    move || {
                        if let Some(song) = imp.player.song() {
                            if song.state() == SwSongState::Recording {
                                imp.duration_label.set_text(&Self::format_duration(
                                    imp.player.recording_duration(),
                                ));
                                return glib::ControlFlow::Continue;
                            }
                        }

                        glib::ControlFlow::Break
                    }
                ),
            );
        }

        fn format_duration(d: u64) -> String {
            let dt = glib::DateTime::from_unix_local(d.try_into().unwrap_or_default()).unwrap();
            dt.format("%M:%S").unwrap_or_default().to_string()
        }
    }
}

glib::wrapper! {
    pub struct SwRecordingIndicator(ObjectSubclass<imp::SwRecordingIndicator>)
        @extends gtk::Widget, adw::Bin;
}

#[gtk::template_callbacks]
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
