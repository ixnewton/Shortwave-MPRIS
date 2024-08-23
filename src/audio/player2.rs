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

use std::cell::{Cell, OnceCell, RefCell};
use std::fs;

use adw::prelude::*;
use glib::clone;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib;

use crate::api::SwStation;
use crate::app::SwApplication;
use crate::audio::backend::*;
use crate::audio::*;
use crate::i18n::*;
use crate::path;
use crate::settings::{settings_manager, Key};
use crate::ui::SwApplicationWindow;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwPlayer)]
    pub struct SwPlayer {
        #[property(get, set=Self::set_station)]
        #[property(name="has-station", get=Self::has_station, type=bool)]
        station: RefCell<Option<SwStation>>,
        #[property(get, builder(SwPlaybackState::default()))]
        state: Cell<SwPlaybackState>,
        #[property(get)]
        last_failure: RefCell<String>,
        #[property(get)]
        #[property(name="has-playing-song", get=Self::has_playing_song, type=bool)]
        playing_song: RefCell<Option<SwSong>>,
        #[property(get)]
        previous_song: RefCell<Option<SwSong>>,
        #[property(get)]
        past_songs: SwSongModel,
        #[property(get, set=Self::set_volume)]
        volume: Cell<f64>,

        pub backend: OnceCell<RefCell<GstreamerBackend>>,
        pub mpris_server: OnceCell<MprisServer>,
        pub inhibit_cookie: Cell<u32>,
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

            // Cleanup recording directory
            let mut path = path::DATA.clone();
            path.push("recording");
            if path.exists() {
                fs::remove_dir_all(path).expect("Could not delete recording directory.");
            }

            // Set how many songs will be saved before they are replaced with newer recordings
            self.past_songs
                .set_max_count(settings_manager::integer(Key::RecorderSaveCount) as u32);

            // Setup Gstreamer backend
            let (sender, receiver) = async_channel::bounded(10);
            self.backend
                .set(RefCell::new(GstreamerBackend::new(sender)))
                .unwrap();

            // Receive change messages from gstreamer backend
            glib::spawn_future_local(clone!(
                #[strong]
                receiver,
                #[weak(rename_to = imp)]
                self,
                async move {
                    while let Ok(message) = receiver.recv().await {
                        imp.process_gst_message(message);
                    }
                }
            ));

            // Restore volume
            let volume = settings_manager::double(Key::PlaybackVolume);
            self.obj().set_volume(volume);

            // mpris
            glib::spawn_future_local(clone!(
                #[weak(rename_to = imp)]
                self,
                async move {
                    match MprisServer::start().await {
                        Ok(mpris_server) => imp.mpris_server.set(mpris_server).unwrap(),
                        Err(err) => error!("Unable to start mpris: {}", err.to_string()),
                    }
                }
            ));
        }
    }

    impl SwPlayer {
        fn has_station(&self) -> bool {
            self.obj().station().is_some()
        }

        fn has_playing_song(&self) -> bool {
            self.obj().playing_song().is_some()
        }

        fn set_station(&self, station: Option<&SwStation>) {
            *self.station.borrow_mut() = station.cloned();
            self.obj().notify_has_station();

            let obj = self.obj();
            obj.stop_playback();

            if let Some(station) = obj.station() {
                if let Some(url) = station.stream_url() {
                    debug!("Start playing new URI: {}", url.to_string());
                    self.backend
                        .get()
                        .unwrap()
                        .borrow_mut()
                        .set_source_uri(url.as_ref());
                } else {
                    let text = i18n("Station cannot be streamed. URL is not valid.");
                    SwApplicationWindow::default().show_notification(&text);
                }
            }
        }

        fn set_volume(&self, volume: f64) {
            debug!("Set volume: {}", &volume);
            self.backend.get().unwrap().borrow().set_volume(volume);
            self.volume.set(volume);
            settings_manager::set_double(Key::PlaybackVolume, volume);
        }

        fn process_gst_message(&self, message: GstreamerChange) -> glib::ControlFlow {
            match message {
                GstreamerChange::Title(title) => {
                    debug!("Stream title has changed to: {}", title);

                    // Stop recording of old song
                    if let Some(song) = self.stop_recording(false) {
                        self.past_songs.add_song(&song);
                    }

                    // Set previous song
                    *self.previous_song.borrow_mut() = self.playing_song.borrow_mut().take();
                    self.obj().notify_previous_song();

                    // Set new song
                    let song = SwSong::new(&title, &self.obj().station().unwrap());
                    self.start_recording(&song);
                    *self.playing_song.borrow_mut() = Some(song);

                    self.obj().notify_playing_song();
                    self.obj().notify_has_playing_song();

                    // Show desktop notification
                    if settings_manager::boolean(Key::Notifications) {
                        // TODO: self.show_song_notification();
                    }
                }
                GstreamerChange::PlaybackState(s) => {
                    let state = match s {
                        PlaybackState::Playing => SwPlaybackState::Playing,
                        PlaybackState::Stopped => SwPlaybackState::Stopped,
                        PlaybackState::Loading => SwPlaybackState::Loading,
                        PlaybackState::Failure(msg) => {
                            *self.last_failure.borrow_mut() = msg;
                            self.obj().notify_last_failure();

                            // Discard recorded data when a failure occurs,
                            // since the song has not been recorded completely.
                            if self.backend.get().unwrap().borrow().is_recording() {
                                self.stop_recording(true);
                                self.clear_song();
                            }

                            SwPlaybackState::Failure
                        }
                    };

                    self.state.set(state);
                    self.obj().notify_state();

                    let app = SwApplication::default();
                    let window = SwApplicationWindow::default();

                    // Inhibit session suspend when playback is active
                    if state == SwPlaybackState::Playing && self.inhibit_cookie.get() == 0 {
                        let cookie = app.inhibit(
                            Some(&window),
                            gtk::ApplicationInhibitFlags::SUSPEND,
                            Some(&i18n("Active Playback")),
                        );
                        self.inhibit_cookie.set(cookie);
                        debug!("Install inhibitor")
                    } else if state != SwPlaybackState::Playing && self.inhibit_cookie.get() != 0 {
                        app.uninhibit(self.inhibit_cookie.get());
                        self.inhibit_cookie.set(0);
                        debug!("Remove inhibitor");
                    }
                }
                GstreamerChange::Volume(volume) => {
                    self.volume.set(volume);
                    self.obj().notify_volume();
                }
            }
            glib::ControlFlow::Continue
        }

        pub fn clear_song(&self) {
            *self.playing_song.borrow_mut() = None;
            *self.previous_song.borrow_mut() = None;
            self.obj().notify_playing_song();
            self.obj().notify_has_playing_song();
            self.obj().notify_previous_song();
        }

        pub fn start_recording(&self, song: &SwSong) {
            // If there is no previous song, we know that the current song is the
            // first song we play from that station. This means that it would be
            // incomplete, as we couldn't record it completely from the beginning.
            if self.obj().previous_song().is_some() {
                let path = song.file().path().unwrap();
                fs::create_dir_all(path.parent().unwrap())
                    .expect("Could not create path for recording");
                song.set_state(SwSongState::Recording);
                self.backend
                    .get()
                    .unwrap()
                    .borrow_mut()
                    .start_recording(path);
            } else {
                debug!(
                    "Song {:?} will not be recorded because it may be incomplete.",
                    song.title()
                );
            }
        }

        /// Returns song object if a complete song has been recorded
        pub fn stop_recording(&self, discard_data: bool) -> Option<SwSong> {
            debug!("Stop recording...");
            let backend = &mut self.backend.get().unwrap().borrow_mut();

            if !backend.is_recording() {
                debug!("No recording, nothing to stop!");
                return None;
            }

            let song = if let Some(song) = self.obj().playing_song() {
                song
            } else {
                warn!("No song available, discard recorded data.");
                backend.stop_recording(true);
                return None;
            };

            let threshold = settings_manager::integer(Key::RecorderSongDurationThreshold);
            let duration: u64 = backend.recording_duration();

            if discard_data {
                debug!("Discard recorded data.");

                backend.stop_recording(true);
                song.set_state(SwSongState::Discarded);

                None
            } else if duration > threshold as u64 {
                debug!("Save recorded data.");

                let duration = backend.recording_duration();
                backend.stop_recording(false);

                song.set_state(SwSongState::Recorded);
                song.set_duration(duration);

                Some(song)
            } else {
                debug!(
                    "Discard recorded data, duration ({} sec) is below threshold ({} sec).",
                    duration, threshold
                );

                backend.stop_recording(true);
                song.set_state(SwSongState::BelowThreshold);

                None
            }
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
        if self.station().is_none() {
            return;
        }

        self.imp()
            .backend
            .get()
            .unwrap()
            .borrow_mut()
            .set_state(gstreamer::State::Playing);
    }

    pub fn stop_playback(&self) {
        let imp = self.imp();

        // Discard recorded data when the stream stops
        imp.stop_recording(true);
        imp.clear_song();

        imp.backend
            .get()
            .unwrap()
            .borrow_mut()
            .set_state(gstreamer::State::Null);
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

    pub fn recording_duration(&self) -> u64 {
        self.imp()
            .backend
            .get()
            .unwrap()
            .borrow()
            .recording_duration()
    }
}

impl Default for SwPlayer {
    fn default() -> Self {
        SwApplication::default().player()
    }
}
