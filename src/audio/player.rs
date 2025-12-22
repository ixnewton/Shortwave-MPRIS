// Shortwave - player.rs
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

use std::cell::{Cell, OnceCell, RefCell};
use std::fs;

use adw::prelude::*;
use glib::clone;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::{gio, glib};

use crate::api::{StationMetadata, SwStation};
use crate::app::SwApplication;
use crate::audio::*;
use crate::config;
use crate::device::{SwCastSender, SwDevice, SwDeviceDiscovery, SwDeviceKind, SwDlnaSender};
use crate::i18n::*;
use crate::path;
use crate::settings::{settings_manager, Key};
use crate::ui::DisplayError;

mod imp {
    use super::*;

    #[derive(PartialEq, Debug)]
    pub enum RecordingStopReason {
        TrackChange,
        StoppedPlayback,
        Cancelled,
        ReachedMaximumDuration,
        StreamFailure,
    }

    impl RecordingStopReason {
        fn discard_data(&self) -> bool {
            // Save recorded data only on track save or when track reaches maximum duration
            *self != Self::TrackChange && *self != Self::ReachedMaximumDuration
        }
    }

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwPlayer)]
    pub struct SwPlayer {
        #[property(get)]
        #[property(name="has-station", get=Self::has_station, type=bool)]
        pub station: RefCell<Option<SwStation>>,
        #[property(get, builder(SwPlaybackState::default()))]
        state: Cell<SwPlaybackState>,
        #[property(get)]
        last_failure: RefCell<String>,
        #[property(get)]
        #[property(name="has-playing-track", get=Self::has_playing_track, type=bool)]
        playing_track: RefCell<Option<SwTrack>>,
        #[property(get)]
        previous_track: RefCell<Option<SwTrack>>,
        #[property(get)]
        past_tracks: SwTrackModel,
        #[property(get, set=Self::set_volume)]
        volume: Cell<f64>,
        #[property(get, set=Self::set_recording_mode, builder(SwRecordingMode::default()))]
        recording_mode: Cell<SwRecordingMode>,

        #[property(get)]
        #[property(name="has-device", get=Self::has_device, type=bool)]
        pub device: RefCell<Option<SwDevice>>,
        #[property(get)]
        pub device_discovery: SwDeviceDiscovery,
        #[property(get)]
        pub cast_sender: SwCastSender,
        pub dlna_sender: OnceCell<SwDlnaSender>,

        pub backend: OnceCell<RefCell<GstreamerBackend>>,
        pub mpris_server: OnceCell<MprisServer>,
        pub gst_sender: OnceCell<async_channel::Sender<GstreamerChange>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwPlayer {
        const NAME: &'static str = "SwPlayer";
        type Type = player::SwPlayer;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwPlayer {
        fn constructed(&self) {
            self.parent_constructed();

            // Setup Gstreamer backend
            let (sender, receiver) = async_channel::bounded(10);
            self.gst_sender.set(sender.clone()).unwrap();
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

            // Remove device on cast disconnect
            self.cast_sender.connect_is_connected_notify(clone!(
                #[weak (rename_to = imp)]
                self,
                move |cs| {
                    if !cs.is_connected() {
                        *imp.device.borrow_mut() = None;
                        imp.obj().notify_device();
                        imp.obj().notify_has_device();
                    }
                }
            ));

            // Sync volume with cast device
            self.obj()
                .bind_property("volume", &self.cast_sender, "volume")
                .sync_create()
                .bidirectional()
                .build();

            // Sync volume with DLNA device (lazy initialization)
            // Note: DLNA sender is created lazily to avoid Tokio runtime issues

            // MPRIS controls
            glib::spawn_future_local(async move {
                MprisServer::start()
                    .await
                    .handle_error("Unable to start MPRIS media controls")
            });

            // Cleanup temporary recording directory
            let mut path = path::DATA.clone();
            path.push("recording");
            if path.exists() {
                fs::remove_dir_all(path).expect("Could not delete recording directory.");
            }

            // Ensure temporary recording directory gsetting is set
            if settings_manager::string(Key::RecordingTrackDirectory).is_empty() {
                settings_manager::set_string(
                    Key::RecordingTrackDirectory,
                    glib::user_special_dir(glib::UserDirectory::Music)
                        .unwrap_or(glib::home_dir())
                        .as_os_str()
                        .to_str()
                        .unwrap()
                        .to_string(),
                );
            }

            // Set how many tracks will be saved before they are replaced with newer recordings
            let max_count = settings_manager::integer(Key::PlaybackPastTracksCount) as u32;
            self.past_tracks.set_max_count(max_count);

            // Bind recording mode setting
            settings_manager::bind_property(Key::RecordingMode, &*self.obj(), "recording-mode");

            glib::timeout_add_seconds_local(
                1,
                clone!(
                    #[weak(rename_to = imp)]
                    self,
                    #[upgrade_or_panic]
                    move || {
                        let mut stop_recording = false;
                        if let Some(track) = imp.obj().playing_track() {
                            let backend = imp.backend.get().unwrap().borrow();
                            if backend.is_recording() {
                                let duration = backend.recording_duration();
                                track.set_duration(duration);

                                // Stop recording if recorded duration exceeds maximum
                                let max = settings_manager::integer(Key::RecordingMaximumDuration);
                                if duration >= max as u64 {
                                    stop_recording = true;
                                }
                            }
                        }

                        if stop_recording {
                            imp.stop_recording(RecordingStopReason::ReachedMaximumDuration);
                        }
                        glib::ControlFlow::Continue
                    }
                ),
            );
        }
    }

    impl SwPlayer {
        fn has_station(&self) -> bool {
            self.obj().station().is_some()
        }

        fn has_playing_track(&self) -> bool {
            self.obj().playing_track().is_some()
        }

        fn has_device(&self) -> bool {
            self.obj().device().is_some()
        }

        pub fn set_volume(&self, volume: f64) {
            if self.volume.get() != volume {
                debug!("Set volume: {}", &volume);
                self.volume.set(volume);

                if self.obj().device().is_none() {
                    self.backend.get().unwrap().borrow().set_volume(volume);
                    settings_manager::set_double(Key::PlaybackVolume, volume);
                } else if let Some(device) = self.obj().device() {
                    // Handle device-specific volume control
                    match device.kind() {
                        SwDeviceKind::Dlna => {
                            debug!("Setting DLNA device volume: {}", volume);
                            if let Err(e) = self.obj().dlna_sender().set_volume_dlna(volume) {
                                warn!("Failed to set DLNA volume: {}", e);
                            }
                        }
                        _ => {
                            // Other device types (Chromecast, etc.) can be handled here
                            self.backend.get().unwrap().borrow().set_volume(volume);
                            settings_manager::set_double(Key::PlaybackVolume, volume);
                        }
                    }
                }
            }
        }

        pub fn set_recording_mode(&self, mode: SwRecordingMode) {
            if self.recording_mode.get() != mode {
                debug!(
                    "Set recording mode: {} -> {}",
                    self.recording_mode.get(),
                    &mode
                );
                self.recording_mode.set(mode);

                if mode == SwRecordingMode::Nothing {
                    self.obj().cancel_recording();
                }
            }
        }

        fn process_gst_message(&self, message: GstreamerChange) -> glib::ControlFlow {
            match message {
                GstreamerChange::Title(title) => self.gst_title_change(&title),
                GstreamerChange::PlaybackState(state) => self.gst_playback_change(&state),
                GstreamerChange::Volume(volume) => self.gst_volume_change(volume),
                GstreamerChange::Failure(f) => self.gst_failure(&f),
            }

            glib::ControlFlow::Continue
        }

        fn gst_title_change(&self, title: &str) {
            debug!("Stream title has changed to: {}", title);
            let track = SwTrack::new(title, &self.obj().station().unwrap());

            // Stop recording of old track
            self.stop_recording(RecordingStopReason::TrackChange);

            // Set previous track
            let mut is_playing_track_from_beginning = false;
            if let Some(track) = self.playing_track.borrow_mut().take() {
                if track.state().include_in_past_tracks() {
                    self.past_tracks.add_track(&track);
                }

                *self.previous_track.borrow_mut() = Some(track);
                self.obj().notify_previous_track();
                is_playing_track_from_beginning = true;
            }

            if self.obj().recording_mode() != SwRecordingMode::Nothing {
                // If there is no previous track, we know that the current track is the
                // first track we play from that station. This means that it would be
                // incomplete, as we couldn't record it completely from the beginning.
                if is_playing_track_from_beginning {
                    self.start_recording(&track);
                } else {
                    track.set_state(SwRecordingState::IdleIncomplete);
                    debug!(
                        "Track {:?} will not be recorded because it may be incomplete.",
                        track.title()
                    );
                }
            }

            // Set new track
            *self.playing_track.borrow_mut() = Some(track.clone());
            self.obj().notify_playing_track();
            self.obj().notify_has_playing_track();

            // Show desktop notification
            if settings_manager::boolean(Key::Notifications) {
                let id = format!("{}.TrackNotification", config::APP_ID);
                SwApplication::default()
                    .send_notification(Some(&id), &self.track_notification(&track));
            }
        }

        fn gst_playback_change(&self, state: &SwPlaybackState) {
            if state == &SwPlaybackState::Failure {
                // Discard recorded data when a failure occurs,
                // since the track has not been recorded completely.
                if self.backend.get().unwrap().borrow().is_recording() {
                    self.stop_recording(RecordingStopReason::StreamFailure);
                    self.reset_track();
                }
            }

            self.state.set(*state);
            self.obj().notify_state();

            // Inhibit session suspend when playback is active
            SwApplication::default().set_inhibit(state == &SwPlaybackState::Playing);
        }

        fn gst_volume_change(&self, volume: f64) {
            if self.obj().device().is_some() {
                return;
            }

            // Check if the volume differs. For some reason gstreamer sends us slightly
            // different floats, so we round up here (only the the first two digits are
            // important for use here).
            let new_val = format!("{:.2}", volume);
            let old_val = format!("{:.2}", self.volume.get());

            if new_val != old_val {
                self.volume.set(volume);
                self.obj().notify_volume();
            }
        }

        fn gst_failure(&self, failure: &str) {
            *self.last_failure.borrow_mut() = failure.to_string();
            self.obj().notify_last_failure();
        }

        /// Unsets the current playing track and adds it to the past played tracks history
        pub fn reset_track(&self) {
            if let Some(track) = self.playing_track.borrow_mut().take() {
                if track.state().include_in_past_tracks() {
                    self.past_tracks.add_track(&track);
                }
            }

            *self.previous_track.borrow_mut() = None;
            self.obj().notify_playing_track();
            self.obj().notify_has_playing_track();
            self.obj().notify_previous_track();
        }

        pub fn start_recording(&self, track: &SwTrack) {
            let path = track.file().path().unwrap();
            fs::create_dir_all(path.parent().unwrap())
                .expect("Could not create path for recording");

            track.set_state(SwRecordingState::Recording);
            self.backend
                .get()
                .unwrap()
                .borrow_mut()
                .start_recording(path);
        }

        pub fn stop_recording(&self, reason: RecordingStopReason) {
            let backend = &mut self.backend.get().unwrap().borrow_mut();

            if !backend.is_recording() {
                debug!("No recording to stop!");
                return;
            }

            let Some(track) = self.obj().playing_track() else {
                warn!("No track for recorded data available, unable to discard.");
                backend.stop_recording(true);
                return;
            };

            let mode = self.obj().recording_mode();
            let minimum_duration = settings_manager::integer(Key::RecordingMinimumDuration);

            let mut duration = backend.recording_duration();
            let mut discard_data = reason.discard_data();

            let mut new_state = if reason.discard_data() {
                duration = 0;
                SwRecordingState::DiscardedCancelled
            } else if reason == RecordingStopReason::ReachedMaximumDuration {
                SwRecordingState::RecordedReachedMaxDuration
            } else {
                SwRecordingState::Recorded
            };

            // Check whether recorded track meets minimum duration
            if new_state.is_recorded() && duration < minimum_duration as u64 {
                debug!(
                    "Discard recorded data, duration ({} sec) is below threshold ({} sec).",
                    duration, minimum_duration
                );

                discard_data = true;
                new_state = SwRecordingState::DiscardedBelowMinDuration;
            }

            track.set_state(new_state);
            track.set_duration(duration);

            // Check whether recorded track should be saved immediately
            let save_track = mode == SwRecordingMode::Everything || track.save_when_recorded();
            if track.state().is_recorded() && save_track {
                track.save().handle_error("Unable to save track");
            }

            debug!(
                "Stop recording track {:?}, reason: {:?}, new state: {}, discard: {}, duration: {}",
                track.title(),
                reason,
                track.state(),
                discard_data,
                track.duration(),
            );
            backend.stop_recording(discard_data);

            if discard_data {
                debug!("Discard recorded data: {}", track.file().parse_name());
                if let Err(err) = track.file().delete(gio::Cancellable::NONE) {
                    warn!("Unable to discard recorded data: {}", err.to_string());
                }
            }
        }

        fn track_notification(&self, track: &SwTrack) -> gio::Notification {
            let notification = gio::Notification::new(&track.title());
            notification.set_body(Some(&track.station().title()));

            let icon = gio::ThemedIcon::new("emblem-music-symbolic");
            notification.set_icon(&icon);

            let target: glib::Variant = track.uuid().into();
            notification.set_default_action_and_target_value("app.show-track", Some(&target));

            if track.state() == SwRecordingState::Recording {
                if self.obj().recording_mode() == SwRecordingMode::Decide {
                    notification.add_button_with_target_value(
                        &i18n("Save Track"),
                        "app.save-track",
                        Some(&target),
                    );
                }

                if self.obj().recording_mode() == SwRecordingMode::Everything
                    || self.obj().recording_mode() == SwRecordingMode::Decide
                {
                    notification.add_button_with_target_value(
                        &i18n("Don't Record"),
                        "app.cancel-recording",
                        Some(&target),
                    );
                }
            }

            notification
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

    fn dlna_sender(&self) -> &SwDlnaSender {
        self.imp().dlna_sender.get_or_init(|| SwDlnaSender::new())
    }

    pub async fn set_station(&self, station: SwStation) {
        debug!("Set station: {}", station.title());
        let imp = self.imp();

        *imp.station.borrow_mut() = Some(station.clone());
        self.notify_station();
        self.notify_has_station();

        self.stop_playback().await;

        if let Some(url) = station.stream_url() {
            debug!("Set new playback URI: {}", url.to_string());
            settings_manager::set_string(
                Key::PlaybackLastStation,
                serde_json::to_string(&station.metadata()).unwrap_or_default(),
            );

            // Only start local GStreamer audio if no remote device is selected
            if self.device().is_none() {
                info!("PLAYER: No remote device selected - starting local audio playback");
                imp.backend
                    .get()
                    .unwrap()
                    .borrow_mut()
                    .set_source_uri(url.as_ref());
            } else {
                info!("PLAYER: Remote device selected - disabling local audio to prevent double playback");
            }

            self.cast_sender()
                .load_media(
                    url.as_ref(),
                    &station
                        .metadata()
                        .favicon
                        .map(|u| u.to_string())
                        .unwrap_or_default(),
                    &station.title(),
                )
                .await
                .handle_error("Unable to load Google Cast media");

            // Load media on DLNA sender if DLNA device is available (no connection needed)
            if self.device().is_some() && self.device().unwrap().kind() == SwDeviceKind::Dlna {
                info!("PLAYER: DLNA device available - loading media");
                self.dlna_sender()
                    .load_media(
                        url.as_ref(),
                        &station
                            .metadata()
                            .favicon
                            .map(|u| u.to_string())
                            .unwrap_or_default(),
                        &station.title(),
                    )
                    .handle_error("Unable to load DLNA media");
            } else {
                info!("PLAYER: No DLNA device available, skipping DLNA media loading");
            }
        } else {
            error!("Station cannot be streamed. URL is not valid.");
        }
    }

    pub async fn start_playback(&self) {
        if self.station().is_none() {
            return;
        }

        // Only start local GStreamer playback if no remote device is selected
        if self.device().is_none() {
            info!("PLAYER: Starting local GStreamer playback");
            self.imp()
                .backend
                .get()
                .unwrap()
                .borrow_mut()
                .set_state(gstreamer::State::Playing);
        } else {
            info!("PLAYER: Remote device active - skipping local GStreamer playback");
        }

        self.cast_sender()
            .start_playback()
            .await
            .handle_error("Unable to start Google Cast playback");

        // Handle remote device state transitions
        if let Some(device) = self.device() {
            if let Some(sender) = self.imp().gst_sender.get() {
                match device.kind() {
                    SwDeviceKind::Cast => {
                        info!("PLAYER: Setting Chromecast playback state to Playing");
                        let _ = sender.send_blocking(GstreamerChange::PlaybackState(SwPlaybackState::Playing));
                    }
                    SwDeviceKind::Dlna => {
                        // DLNA is handled below with media loading
                    }
                    _ => {}
                }
            }
        }

        // Start DLNA playback if DLNA device is available
        if self.device().is_some() && self.device().unwrap().kind() == SwDeviceKind::Dlna {
            info!("PLAYER: Starting DLNA playback");
            
            // Load media first, then start playback
            if let Some(station) = self.station() {
                if let Some(url) = station.stream_url() {
                    self.dlna_sender()
                        .load_media(
                            url.as_ref(),
                            &station
                                .metadata()
                                .favicon
                                .map(|u| u.to_string())
                                .unwrap_or_default(),
                            &station.title(),
                        )
                        .handle_error("Unable to load DLNA media");
                        
                    // Start DLNA playback and set state to Playing
                    self.dlna_sender()
                        .start_playback()
                        .handle_error("Unable to start DLNA playback");
                    
                    // Manually set player state to Playing since we skip GStreamer
                    info!("PLAYER: Setting DLNA playback state to Playing");
                    if let Some(sender) = self.imp().gst_sender.get() {
                        let _ = sender.send_blocking(GstreamerChange::PlaybackState(SwPlaybackState::Playing));
                    }
                }
            }
        }
    }

    pub async fn stop_playback(&self) {
        println!("=== STOP BUTTON PRESSED ===");
        info!("PLAYER: stop_playback() called - checking device type");
        let imp = self.imp();

        // Save device info before stopping to prevent it from being cleared
        let device_before_stop = self.device().clone();
        let device_kind_before_stop = device_before_stop.as_ref().map(|d| d.kind());
        info!("PLAYER: Device before stop: {:?}", device_kind_before_stop);

        // Discard recorded data when the stream stops
        imp.stop_recording(imp::RecordingStopReason::StoppedPlayback);
        imp.reset_track();

        imp.backend
            .get()
            .unwrap()
            .borrow_mut()
            .set_state(gstreamer::State::Null);

        self.cast_sender()
            .stop_playback()
            .await
            .handle_error("Unable to stop Google Cast playback");

        // Stop DLNA playback if DLNA device is available
        let device_kind = self.device().map(|d| d.kind());
        info!("PLAYER: Device kind after state change: {:?}", device_kind);
        
        // Use the saved device info if device was cleared
        let final_device = if self.device().is_none() {
            device_before_stop
        } else {
            self.device()
        };
        
        if let Some(device) = final_device {
            if device.kind() == SwDeviceKind::Dlna {
                info!("PLAYER: Stopping DLNA playback");
                self.dlna_sender()
                    .stop_playback()
                    .handle_error("Unable to stop DLNA playback");
            } else {
                info!("PLAYER: Not stopping DLNA - device is not DLNA kind: {:?}", device.kind());
            }
        } else {
            info!("PLAYER: Not stopping DLNA - no device available");
        }
    }

    pub async fn toggle_playback(&self) {
        if self.state() == SwPlaybackState::Playing || self.state() == SwPlaybackState::Loading {
            self.stop_playback().await;
        } else if self.state() == SwPlaybackState::Stopped
            || self.state() == SwPlaybackState::Failure
        {
            self.start_playback().await;
        }
    }

    pub fn cancel_recording(&self) {
        let imp = self.imp();
        imp.stop_recording(imp::RecordingStopReason::Cancelled);
    }

    pub fn restore_state(&self) {
        let imp = self.imp();

        // Restore volume
        let volume = settings_manager::double(Key::PlaybackVolume);
        imp.set_volume(volume);
        self.notify_volume();

        // Restore last played station
        let json = settings_manager::string(Key::PlaybackLastStation);
        if json.is_empty() {
            return;
        }

        match serde_json::from_str::<StationMetadata>(&json) {
            Ok(station_metadata) => {
                let library_model = SwApplication::default().library().model();

                let station =
                    if let Some(station) = library_model.station(&station_metadata.stationuuid) {
                        // Try to reuse the station object from the library,
                        // since it's possible that it has a custom cover set
                        station
                    } else {
                        SwStation::new(
                            &station_metadata.stationuuid,
                            false,
                            station_metadata.clone(),
                            None,
                        )
                    };

                glib::spawn_future_local(clone!(
                    #[weak(rename_to = obj)]
                    self,
                    #[weak]
                    station,
                    async move {
                        obj.set_station(station).await;
                    }
                ));
            }
            Err(e) => warn!("Unable to restore last played station: {}", e.to_string()),
        }
    }

    pub async fn connect_device(&self, device: &SwDevice) -> Result<(), Box<dyn std::error::Error>> {
        let result = match device.kind() {
            SwDeviceKind::Cast => {
                self.cast_sender()
                    .connect(&device.address())
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            }
            SwDeviceKind::Dlna => {
                // For DLNA, NO connection needed at all - just store device
                info!("PLAYER: Storing DLNA device - no connection required");
                // Store device URL in DLNA sender for later use
                self.dlna_sender().imp().device.borrow_mut().replace(device.address().clone());
                Ok(()) // Always succeed for DLNA
            }
        };

        if result.is_ok() {
            *self.imp().device.borrow_mut() = Some(device.clone());
            self.notify_has_device();
            self.notify_device();

            if self.state() == SwPlaybackState::Playing || self.state() == SwPlaybackState::Loading
            {
                match device.kind() {
                    SwDeviceKind::Cast => {
                        self.cast_sender().start_playback().await?;
                    }
                    SwDeviceKind::Dlna => {
                        // For DLNA, don't start playback here - wait for play button
                        info!("PLAYER: DLNA device stored - playback will start when play button pressed");
                    }
                }
            }
        }

        result
    }

    pub async fn disconnect_device(&self) {
        if let Some(device) = self.device() {
            match device.kind() {
                SwDeviceKind::Cast => self.cast_sender().disconnect().await,
                SwDeviceKind::Dlna => self.dlna_sender().disconnect(),
            };

            *self.imp().device.borrow_mut() = None;
            self.notify_has_device();
            self.notify_device();

            // Restore previous gstreamer volume
            let volume = {
                let backend = self.imp().backend.get().unwrap().borrow_mut();
                backend.set_mute(false);
                backend.volume()
            };
            debug!("Restore previous volume: {}", volume);
            self.set_volume(volume);
        }
    }

    pub fn track_by_uuid(&self, uuid: &str) -> Option<SwTrack> {
        if let Some(track) = self.playing_track() {
            if track.uuid() == uuid {
                return Some(track.clone());
            }
        }

        self.past_tracks().track_by_uuid(uuid)
    }
}

impl Default for SwPlayer {
    fn default() -> Self {
        Self::new()
    }
}
