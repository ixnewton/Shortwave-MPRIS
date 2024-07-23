// Shortwave - player.rs
// Copyright (C) 2021-2024  Felix Häcker <haeckerfelix@gnome.org>
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
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use async_channel::Sender;
use glib::clone;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib::Enum;
use gtk::{gio, glib};

use crate::api::SwStation;
use crate::app::Action;
use crate::audio::backend::*;
#[cfg(unix)]
use crate::audio::controller::MprisController;
use crate::audio::controller::{
    Controller, GCastController, InhibitController, MiniController, SidebarController,
    ToolbarController,
};
use crate::audio::{GCastDevice, Song};
use crate::i18n::*;
use crate::settings::{settings_manager, Key};
use crate::ui::SwApplicationWindow;
use crate::{config, path};

#[derive(Display, Copy, Debug, Clone, EnumString, Eq, PartialEq, Enum)]
#[repr(u32)]
#[enum_type(name = "SwPlaybackState")]
#[derive(Default)]
pub enum SwPlaybackState {
    #[default]
    Stopped,
    Playing,
    Loading,
    Failure,
}

mod imp {

    use crate::audio::PlaybackState;

    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwPlayer)]
    pub struct SwPlayer {
        #[property(get, builder(SwPlaybackState::default()))]
        state: Cell<SwPlaybackState>,
        #[property(get, set=Self::set_station)]
        station: RefCell<Option<SwStation>>,
        #[property(get, set=Self::set_volume)]
        volume: Cell<f64>,

        pub backend: RefCell<Backend>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwPlayer {
        const NAME: &'static str = "SwPlayer";
        type Type = super::SwPlayer;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwPlayer {
        fn constructed(&self) {
            self.parent_constructed();

            let receiver = self.backend.borrow_mut().gstreamer_receiver.take().unwrap();

            glib::spawn_future_local(clone!(
                #[strong]
                receiver,
                #[weak(rename_to = this)]
                self,
                async move {
                    while let Ok(message) = receiver.recv().await {
                        this.process_gst_message(message);
                    }
                }
            ));
        }
    }

    impl SwPlayer {
        fn set_station(&self, station: Option<&SwStation>) {
            let obj = self.obj();

            *self.station.borrow_mut() = station.cloned();
            obj.stop_playback();

            // Reset song title
            // TODO: self.song_title.borrow_mut().reset();

            if let Some(station) = obj.station() {
                let metadata = station.metadata();

                // We try playing from `url_resolved` first, which is the pre-resolved
                // URL from the API. However, for local stations, we don't do that, so
                // `url_resolved` will be `None`. In that case we just use `url`, which
                // can also be a potential fallback in case the API misses the resolved
                // URL for some reason.
                if let Some(url) = metadata.url_resolved.or(metadata.url) {
                    debug!("Start playing new URI: {}", url.to_string());
                    self.backend
                        .borrow_mut()
                        .gstreamer
                        .new_source_uri(url.as_ref());
                } else {
                    let text = i18n("Station cannot be streamed. URL is not valid.");
                    SwApplicationWindow::default().show_notification(&text);
                }
            }
        }

        fn set_volume(&self, value: f64) {
            debug!("Set volume: {}", &value);
            self.volume.set(value);

            self.backend.borrow().gstreamer.set_volume(value);
            settings_manager::set_double(Key::PlaybackVolume, value);
        }

        fn process_gst_message(&self, message: GstreamerMessage) -> glib::ControlFlow {
            match message {
                GstreamerMessage::SongTitleChanged(title) => {
                    let backend = &mut self.backend.borrow_mut();
                    debug!("Song title has changed to: \"{}\"", title);

                    // If we're already recording something, we need to stop it first.
                    if backend.gstreamer.is_recording() {
                        let threshold: i64 =
                            settings_manager::integer(Key::RecorderSongDurationThreshold).into();
                        let duration: i64 = backend.gstreamer.current_recording_duration();
                        if duration > threshold {
                            backend.gstreamer.stop_recording(false);

                            let duration = Duration::from_secs(duration.try_into().unwrap());
                            // TODO
                            /*
                                let song = self
                                    .song_title
                                    .borrow()
                                    .create_song(duration)
                                    .expect("Unable to create new song");
                                backend.song.add_song(song);
                            */
                        } else {
                            debug!("Discard recorded data, song duration ({} sec) is below threshold ({} sec).", duration, threshold);
                            backend.gstreamer.stop_recording(true);
                        }
                    }

                    // Set new song title
                    // TODO: self.song_title.borrow_mut().set_current_title(title.clone());

                    // Start recording new song
                    // We don't start recording the "first" detected song, since it is going to be
                    // incomplete
                    // TODO
                    /*
                    if !self.song_title.borrow().is_first_song() {
                        backend.gstreamer.start_recording(
                            self.song_title
                                .borrow()
                                .current_path()
                                .expect("Unable to get song path"),
                        );
                    } else {
                        debug!("Song will not be recorded because it may be incomplete (first song for this station).")
                    }
                     */

                    // Show desktop notification
                    if settings_manager::boolean(Key::Notifications) {
                        // TODO: self.show_song_notification();
                    }
                }
                GstreamerMessage::PlaybackStateChanged(s) => {
                    let state = match s {
                        PlaybackState::Playing => SwPlaybackState::Playing,
                        PlaybackState::Stopped => SwPlaybackState::Stopped,
                        PlaybackState::Loading => SwPlaybackState::Loading,
                        PlaybackState::Failure(_) => SwPlaybackState::Failure,
                    };
                    self.state.set(state);
                    self.obj().notify_state();

                    // Discard recorded data when a failure occurs,
                    // since the song has not been recorded completely.
                    if self.backend.borrow().gstreamer.is_recording()
                        && matches!(state, SwPlaybackState::Failure)
                    {
                        // TODO
                        self.backend.borrow_mut().gstreamer.stop_recording(true);
                    }
                }
            }
            glib::ControlFlow::Continue
        }
    }
}

glib::wrapper! {
    pub struct SwPlayer(ObjectSubclass<imp::SwPlayer>);
}

impl SwPlayer {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn start_playback(&self) {
        self.imp()
            .backend
            .borrow_mut()
            .gstreamer
            .set_state(gstreamer::State::Playing);
    }

    pub fn stop_playback(&self) {
        let mut backend = self.imp().backend.borrow_mut();

        // Discard recorded data when the stream stops
        if backend.gstreamer.is_recording() {
            backend.gstreamer.stop_recording(true);
        }

        // Reset song title
        // TODO: self.song_title.borrow_mut().reset();

        backend.gstreamer.set_state(gstreamer::State::Null);
    }

    pub fn toggle_playback(&self) {
        if self.state() == SwPlaybackState::Playing || self.state() == SwPlaybackState::Loading {
            self.stop_playback();
        } else if self.state() == SwPlaybackState::Stopped
            || self.state() == SwPlaybackState::Failure
        {
            self.start_playback();
        }
    }
}

impl Default for SwPlayer {
    fn default() -> Self {
        Self::new()
    }
}
