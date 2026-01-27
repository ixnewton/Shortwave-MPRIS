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

                // Determine which volume key to use based on device type
                let volume_key = if self.obj().device().is_none() {
                    Key::PlaybackVolumeLocal
                } else if let Some(device) = self.obj().device() {
                    match device.kind() {
                        SwDeviceKind::Cast => Key::PlaybackVolumeCast,
                        SwDeviceKind::Dlna => Key::PlaybackVolumeDlna,
                    }
                } else {
                    Key::PlaybackVolumeLocal
                };

                if self.obj().device().is_none() {
                    self.backend.get().unwrap().borrow().set_volume(volume);
                    settings_manager::set_double(volume_key, volume);
                } else if let Some(device) = self.obj().device() {
                    // Handle device-specific volume control
                    match device.kind() {
                        SwDeviceKind::Dlna => {
                            debug!("Setting DLNA device volume: {}", volume);
                            if let Err(e) = self.obj().dlna_sender().set_volume_dlna(volume) {
                                warn!("Failed to set DLNA volume: {}", e);
                            } else {
                                // Only save volume if DLNA device accepted it
                                settings_manager::set_double(volume_key, volume);
                            }
                        }
                        SwDeviceKind::Cast => {
                            debug!("Setting Cast device volume: {}", volume);
                            self.backend.get().unwrap().borrow().set_volume(volume);
                            settings_manager::set_double(volume_key, volume);
                        }
                        _ => {
                            // Fallback for unknown device types
                            self.backend.get().unwrap().borrow().set_volume(volume);
                            settings_manager::set_double(volume_key, volume);
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
        self.set_station_with_playback(station, true).await;
    }

    pub async fn set_station_with_playback(&self, station: SwStation, start_playback: bool) {
        debug!("Set station: {} (start_playback: {})", station.title(), start_playback);
        let imp = self.imp();

        // Check Chromecast compatibility BEFORE updating station metadata
        if let Some(url) = station.stream_url() {
            let url_str = url.to_string();
            
            // Check Chromecast compatibility if a cast device is connected
            // If device was cancelled, it will be None and no check will be performed
            if let Some(device) = self.device() {
                if device.kind() == SwDeviceKind::Cast {
                    #[cfg(feature = "dlna-debug")]
                    println!("🔍 STATION: Cast device connected - checking compatibility");
                    
                    // Check for incompatible formats
                    if url_str.contains(".m3u8") || url_str.contains("/live/") || url_str.contains("playlist") {
                        warn!("PLAYER: New station incompatible with Chromecast - showing error but keeping current stream");
                        
                        // Show error notification to user but don't change station
                        if let Some(sender) = imp.gst_sender.get() {
                            let _ = sender.send_blocking(GstreamerChange::Failure("Radio stream incompatible with cast device!".to_string()));
                        }
                        
                        // Don't update station or stop playback - keep current Chromecast stream playing
                        return;
                    }
                }
            }
        }

        // If we get here, the station is compatible or no Chromecast is connected
        // Sequence for play-new: stop current playback -> update station UI -> load/start output.
        self.stop_playback().await;

        *imp.station.borrow_mut() = Some(station.clone());
        self.notify_station();
        self.notify_has_station();

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
                
                // Reapply saved volume after setting URI to ensure it's properly set in the audio system
                let device_kind = self.device().map(|d| d.kind());
                let volume_key = match device_kind {
                    Some(SwDeviceKind::Cast) => Key::PlaybackVolumeCast,
                    Some(SwDeviceKind::Dlna) => Key::PlaybackVolumeDlna,
                    None => Key::PlaybackVolumeLocal,
                };
                
                let saved_volume = settings_manager::double(volume_key);
                let saved_volume = if saved_volume <= 0.0 {
                    info!("PLAYER: No saved volume found for {:?}, using default 50%", device_kind);
                    0.5
                } else {
                    info!("PLAYER: Restored saved volume {} for {:?} when setting new station", saved_volume, device_kind);
                    saved_volume
                };
                
                info!("PLAYER: Applying saved volume {} after setting URI", saved_volume);
                self.set_volume(saved_volume);
                imp.backend.get().unwrap().borrow().set_volume(saved_volume);
                
                // Start playback immediately after setting the URI if requested
                if start_playback {
                    info!("PLAYER: Starting playback immediately after setting URI");
                    imp.backend
                        .get()
                        .unwrap()
                        .borrow_mut()
                        .set_state(gstreamer::State::Playing);
                } else {
                    info!("PLAYER: Not starting playback - only loading station");
                }
            } else {
                info!("PLAYER: Remote device selected - disabling local audio to prevent double playback");
                
                // Restore saved volume for remote devices when setting new station
                let device_kind = self.device().map(|d| d.kind());
                if let Some(kind) = device_kind {
                    let volume_key = match kind {
                        SwDeviceKind::Cast => Key::PlaybackVolumeCast,
                        SwDeviceKind::Dlna => Key::PlaybackVolumeDlna,
                    };
                    
                    let saved_volume = settings_manager::double(volume_key);
                    let saved_volume = if saved_volume <= 0.0 {
                        info!("PLAYER: No saved volume found for {:?}, using default 50%", kind);
                        0.5
                    } else {
                        info!("PLAYER: Restored saved volume {} for {:?} when setting new station", saved_volume, kind);
                        saved_volume
                    };
                    
                    info!("PLAYER: Applying saved volume {} for remote device {:?}", saved_volume, kind);
                    self.set_volume(saved_volume);
                }
            }

            // Start playback on remote devices if requested
            if start_playback && self.device().is_some() {
                // Set Loading state before starting remote playback
                if let Some(sender) = imp.gst_sender.get() {
                    let _ = sender.send_blocking(GstreamerChange::PlaybackState(SwPlaybackState::Loading));
                }
                info!("PLAYER: Set Loading state for remote device playback");
                
                // Start playback which will handle state transitions
                self.start_playback().await;
            }
        } else {
            error!("Station cannot be streamed. URL is not valid.");
            // Set player state to failure when no valid URL is available
            if let Some(sender) = imp.gst_sender.get() {
                let _ = sender.send_blocking(GstreamerChange::Failure(i18n("Station cannot be streamed. URL is not valid.")));
            }
        }
    }

    pub async fn start_playback(&self) {
        if self.station().is_none() {
            return;
        }

        // Set loading state immediately to show spinner
        if let Some(sender) = self.imp().gst_sender.get() {
            let _ = sender.send_blocking(GstreamerChange::PlaybackState(SwPlaybackState::Loading));
        }
        info!("PLAYER: Set playback state to Loading - showing spinner");

        // Get device kind for volume management
        let device_kind = self.device().map(|d| d.kind());
        
        // Restore saved volume for the specific device type
        let volume_key = match device_kind {
            Some(SwDeviceKind::Cast) => Key::PlaybackVolumeCast,
            Some(SwDeviceKind::Dlna) => Key::PlaybackVolumeDlna,
            None => Key::PlaybackVolumeLocal,
        };
        
        let saved_volume = settings_manager::double(volume_key);
        let saved_volume = if saved_volume <= 0.0 {
            info!("PLAYER: No saved volume found for {:?}, using default 50%", device_kind);
            0.5
        } else {
            info!("PLAYER: Restored saved volume {} for {:?}", saved_volume, device_kind);
            saved_volume
        };

        // Only start local GStreamer playback if no remote device is selected
        if self.device().is_none() {
            info!("PLAYER: Starting local GStreamer playback");
            self.imp()
                .backend
                .get()
                .unwrap()
                .borrow_mut()
                .set_state(gstreamer::State::Playing);
            
            // Set volume AFTER state transition to prevent GStreamer from resetting it
            info!("PLAYER: Setting volume {} after GStreamer state transition", saved_volume);
            self.set_volume(saved_volume);
            self.imp()
                .backend
                .get()
                .unwrap()
                .borrow()
                .set_volume(saved_volume);
        } else {
            info!("PLAYER: Remote device active - setting volume for remote device");
            self.set_volume(saved_volume);
        }

        // Handle remote device playback
        if let Some(device) = self.device() {
            match device.kind() {
                SwDeviceKind::Cast => {
                    self.cast_sender()
                        .start_playback()
                        .await
                        .handle_error("Unable to start Google Cast playback");
                    
                    // Set Playing state after Cast command completes
                    info!("PLAYER: Setting Chromecast playback state to Playing");
                    if let Some(sender) = self.imp().gst_sender.get() {
                        let _ = sender.send_blocking(GstreamerChange::PlaybackState(SwPlaybackState::Playing));
                    }
                }
                SwDeviceKind::Dlna => {
                    #[cfg(feature = "dlna-debug")]
                    {
                        println!("🟢 PLAY: === DLNA PLAYBACK REQUESTED ===");
                        println!("🟢 PLAY: DLNA Device Details:");
                        println!("🟢 PLAY:   - Name: {}", device.name());
                        println!("🟢 PLAY:   - Address: {}", device.address());
                        println!("🟢 PLAY:   - Saved Volume: {}", saved_volume);
                    }
                    info!("PLAYER: === DLNA PLAYBACK REQUESTED ===");
                    info!("PLAYER: DLNA Device Details:");
                    info!("PLAYER:   - Name: {}", device.name());
                    info!("PLAYER:   - Address: {}", device.address());
                    info!("PLAYER:   - Saved Volume: {}", saved_volume);
                    
                    // Execute DLNA playback sequence
                    let dlna_result = self.start_dlna_playback_sequence(saved_volume).await;
                    
                    // Set final state based on result
                    if let Some(sender) = self.imp().gst_sender.get() {
                        match dlna_result {
                            Ok(_) => {
                                info!("PLAYER: ✅ DLNA playback sequence completed - setting Playing state");
                                let _ = sender.send_blocking(GstreamerChange::PlaybackState(SwPlaybackState::Playing));
                            }
                            Err(e) => {
                                error!("PLAYER: ❌ DLNA playback sequence failed: {} - setting Stopped state", e);
                                let _ = sender.send_blocking(GstreamerChange::PlaybackState(SwPlaybackState::Stopped));
                                let _ = sender.send_blocking(GstreamerChange::Failure(format!("DLNA playback failed: {}", e)));
                            }
                        }
                    }
                    info!("PLAYER: === DLNA PLAYBACK COMPLETED ===");
                }
            }
        }
    }

    async fn start_dlna_playback_sequence(&self, saved_volume: f64) -> Result<(), Box<dyn std::error::Error>> {
        // Apply saved volume to DLNA device
        info!("PLAYER: Step 1 - Setting DLNA device volume to {}", saved_volume);
        if let Err(e) = self.dlna_sender().set_volume_dlna(saved_volume) {
            warn!("PLAYER: ⚠️ Failed to set DLNA volume: {}", e);
        } else {
            info!("PLAYER: ✅ Volume set successfully");
        }
        
        // Check if FFmpeg proxy is already running
        let dlna_sender = self.dlna_sender();
        let proxy_running = dlna_sender.imp().ffmpeg_process.borrow().is_some();
        
        info!("PLAYER: Step 2 - Checking FFmpeg proxy status");
        info!("PLAYER: FFmpeg proxy running: {}", proxy_running);
        
        // Show current proxy configuration
        info!("PLAYER: Current Proxy Configuration:");
        info!("PLAYER:   - Local IP: {}", dlna_sender.imp().local_ip.borrow());
        info!("PLAYER:   - Proxy Port: {}", dlna_sender.imp().ffmpeg_port.get());
        info!("PLAYER:   - Original URL: {}", dlna_sender.imp().original_stream_url.borrow());
        
        if proxy_running {
            info!("PLAYER: ✅ FFmpeg proxy already running - sending play command only");
            // Only send play command if proxy is already running
            info!("PLAYER: Step 3 - Sending Play command to DLNA device");
            dlna_sender.start_playback()?;
            info!("PLAYER: ✅ Step 3 COMPLETE - Play command sent successfully");
        } else {
            info!("PLAYER: ℹ️ FFmpeg proxy not running - starting full setup");
            info!("PLAYER: Step 3 - Starting FFmpeg proxy and sending to device");
            
            if let Some(station) = self.station() {
                info!("PLAYER: Station Details:");
                info!("PLAYER:   - Title: {}", station.title());
                info!("PLAYER:   - UUID: {}", station.uuid());
                
                if let Some(url) = station.stream_url() {
                    info!("PLAYER: Original Stream URL: {}", url);
                    
                    dlna_sender.load_media(
                        url.as_ref(),
                        &station
                            .metadata()
                            .favicon
                            .map(|u| u.to_string())
                            .unwrap_or_default(),
                        &station.title(),
                    )?;
                    info!("PLAYER: ✅ Step 3 COMPLETE - FFmpeg proxy started and URL sent to device");
                } else {
                    error!("PLAYER: ❌ No stream URL available for station");
                    return Err("No stream URL available".into());
                }
            } else {
                error!("PLAYER: ❌ No station loaded - cannot start DLNA playback");
                return Err("No station loaded".into());
            }
        }
        
        Ok(())
    }

    pub async fn toggle_playback(&self) {
        #[cfg(feature = "dlna-debug")]
        {
            println!("🔵 TOGGLE: toggle_playback() called");
            println!("🔵 TOGGLE: Current state: {:?}", self.state());
        }
        
        if self.state() == SwPlaybackState::Playing || self.state() == SwPlaybackState::Loading {
            #[cfg(feature = "dlna-debug")]
            println!("🔵 TOGGLE: State is Playing/Loading - calling stop_playback()");
            self.stop_playback().await;
        } else if self.state() == SwPlaybackState::Stopped
            || self.state() == SwPlaybackState::Failure
        {
            #[cfg(feature = "dlna-debug")]
            println!("🔵 TOGGLE: State is Stopped/Failure - calling start_playback()");
            self.start_playback().await;
        }
        
        #[cfg(feature = "dlna-debug")]
        println!("🔵 TOGGLE: toggle_playback() completed");
    }

    pub async fn stop_playback(&self) {
        #[cfg(feature = "dlna-debug")]
        {
            println!("=== STOP BUTTON PRESSED ===");
            println!("🔴 STOP: stop_playback() called");
        }
        info!("PLAYER: stop_playback() called");
        let imp = self.imp();

        // Save device info before stopping
        let device_before_stop = self.device();
        let device_kind = device_before_stop.as_ref().map(|d| d.kind());
        #[cfg(feature = "dlna-debug")]
        println!("🔴 STOP: Device type: {:?}", device_kind);
        info!("PLAYER: Device before stop: {:?}", device_kind);

        // Discard recorded data when the stream stops
        #[cfg(feature = "dlna-debug")]
        println!("🔴 STOP: Stopping recording and resetting track");
        imp.stop_recording(imp::RecordingStopReason::StoppedPlayback);
        imp.reset_track();

        // Stop GStreamer backend
        #[cfg(feature = "dlna-debug")]
        println!("🔴 STOP: Setting GStreamer to Null state");
        imp.backend
            .get()
            .unwrap()
            .borrow_mut()
            .set_state(gstreamer::State::Null);

        // Stop Cast playback
        #[cfg(feature = "dlna-debug")]
        println!("🔴 STOP: Stopping Cast sender");
        self.cast_sender()
            .stop_playback()
            .await
            .handle_error("Unable to stop Google Cast playback");

        // Stop DLNA playback if DLNA device is active
        if let Some(device) = device_before_stop {
            match device.kind() {
                SwDeviceKind::Dlna => {
                    #[cfg(feature = "dlna-debug")]
                    {
                        println!("🔴 STOP: DLNA device detected - stopping DLNA playback");
                        println!("🔴 STOP: Calling dlna_sender().stop_playback()");
                    }
                    info!("PLAYER: Stopping DLNA playback");
                    
                    if let Err(e) = self.dlna_sender().stop_playback() {
                        #[cfg(feature = "dlna-debug")]
                        println!("🔴 STOP: ❌ Failed to stop DLNA playback: {}", e);
                        warn!("PLAYER: Failed to stop DLNA playback: {}", e);
                    } else {
                        #[cfg(feature = "dlna-debug")]
                        println!("🔴 STOP: ✅ DLNA playback stopped successfully");
                    }
                    
                    #[cfg(feature = "dlna-debug")]
                    println!("🔴 STOP: Calling dlna_sender().stop_ffmpeg_server()");
                    self.dlna_sender().stop_ffmpeg_server();
                    #[cfg(feature = "dlna-debug")]
                    println!("🔴 STOP: ✅ FFmpeg server stopped");
                }
                SwDeviceKind::Cast => {
                    #[cfg(feature = "dlna-debug")]
                    println!("🔴 STOP: Cast device - already stopped via cast_sender()");
                    info!("PLAYER: Cast device stopped");
                }
            }
        } else {
            #[cfg(feature = "dlna-debug")]
            println!("🔴 STOP: No device active - local playback stopped");
            info!("PLAYER: No device active - local playback stopped");
        }

        #[cfg(feature = "dlna-debug")]
        println!("=== STOP PLAYBACK COMPLETED ===");
        info!("PLAYER: stop_playback() completed");
    }

    pub fn cancel_recording(&self) {
        let imp = self.imp();
        imp.stop_recording(imp::RecordingStopReason::Cancelled);
    }

    pub fn restore_state(&self) {
        let imp = self.imp();

        // Restore volume with a small delay to ensure UI bindings are ready
        let volume = settings_manager::double(Key::PlaybackVolume);
        debug!("PLAYER: Restoring volume from settings: {}", volume);
        
        // Apply volume immediately
        imp.set_volume(volume);
        debug!("PLAYER: Volume set to: {}", self.volume());
        self.notify_volume();
        
        // Also ensure volume is applied after UI is ready
        glib::spawn_future_local(clone!(
            #[weak(rename_to = obj)]
            self,
            #[weak(rename_to = imp)]
            imp,
            #[strong]
            volume,
            async move {
                glib::timeout_future(std::time::Duration::from_millis(100)).await;
                debug!("PLAYER: Reapplying volume after UI delay: {}", volume);
                imp.set_volume(volume);
                obj.notify_volume();
            }
        ));

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
                        obj.set_station_with_playback(station, false).await;
                    }
                ));
            }
            Err(e) => warn!("Unable to restore last played station: {}", e.to_string()),
        }
    }

    pub async fn connect_device(&self, device: &SwDevice) -> Result<(), Box<dyn std::error::Error>> {
        // Check Chromecast compatibility before connecting
        if device.kind() == SwDeviceKind::Cast {
            if let Some(station) = self.station() {
                if let Some(url) = station.stream_url() {
                    let url_str = url.to_string();
                    
                    // Check for incompatible formats
                    if url_str.contains(".m3u8") || url_str.contains("/live/") || url_str.contains("playlist") {
                        let error_msg = "Radio stream incompatible with cast device!";
                        warn!("PLAYER: Chromecast compatibility check failed: {}", error_msg);
                        return Err(error_msg.into());
                    }
                } else {
                    let error_msg = "No stream URL available for cast device!";
                    warn!("PLAYER: Chromecast compatibility check failed: {}", error_msg);
                    return Err(error_msg.into());
                }
            } else {
                let error_msg = "Please select a radio station before connecting to cast device!";
                warn!("PLAYER: Chromecast compatibility check failed: {}", error_msg);
                return Err(error_msg.into());
            }
        }

        let result = match device.kind() {
            SwDeviceKind::Cast => {
                self.cast_sender()
                    .connect(&device.address())
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            }
            SwDeviceKind::Dlna => {
                // For DLNA, just connect to fetch service URLs - don't start FFmpeg yet
                info!("PLAYER: === DLNA DEVICE SELECTION STARTED ===");
                info!("PLAYER: DLNA Device Details:");
                info!("PLAYER:   - Name: {}", device.name());
                info!("PLAYER:   - Address: {}", device.address());
                info!("PLAYER:   - Kind: {:?}", device.kind());
                
                // Stop any existing FFmpeg instances to ensure clean state
                info!("PLAYER: Step 1 - Stopping any existing FFmpeg instances");
                self.dlna_sender().stop_ffmpeg_server();
                
                info!("PLAYER: Step 2 - Connecting to DLNA device to fetch service URLs");
                match self.dlna_sender().connect(&device.address()) {
                    Ok(_) => {
                        info!("PLAYER: ✅ Step 2 COMPLETE - DLNA device connected successfully");
                        info!("PLAYER: Service URLs fetched and stored");
                        
                        // Show current DLNA sender state
                        let dlna_sender = self.dlna_sender();
                        info!("PLAYER: Current DLNA Sender State:");
                        info!("PLAYER:   - Device URL: {:?}", dlna_sender.imp().device.borrow());
                        info!("PLAYER:   - AV Transport URL: {:?}", dlna_sender.imp().av_transport_url.borrow());
                        info!("PLAYER:   - Rendering Control URL: {:?}", dlna_sender.imp().rendering_control_url.borrow());
                        
                        // NOTE: FFmpeg proxy will be started when play button is pressed
                        // This keeps device selection fast and non-blocking
                        if let Some(station) = self.station() {
                            info!("PLAYER: ℹ️ Station loaded - FFmpeg proxy will start when play is pressed");
                            info!("PLAYER: Station: {}", station.title());
                        } else {
                            info!("PLAYER: ℹ️ No station loaded - select station and press play to start streaming");
                        }
                        
                        info!("PLAYER: === DLNA DEVICE SELECTION COMPLETED ===");
                        Ok(())
                    }
                    Err(e) => {
                        error!("PLAYER: ❌ Step 1 FAILED - Failed to connect to DLNA device: {}", e);
                        Err(e)
                    }
                }
            }
        };

        if result.is_ok() {
            *self.imp().device.borrow_mut() = Some(device.clone());
            self.notify_has_device();
            self.notify_device();

            // If something is already playing locally, start playback on the device immediately
            if self.state() == SwPlaybackState::Playing || self.state() == SwPlaybackState::Loading
            {
                // Stop local GStreamer audio first to ensure clean transition
                info!("PLAYER: Stopping local audio playback");
                self.imp()
                    .backend
                    .get()
                    .unwrap()
                    .borrow_mut()
                    .set_state(gstreamer::State::Null);

                match device.kind() {
                    SwDeviceKind::Cast => {
                        info!("PLAYER: Starting Cast playback - stopping local audio");
                        self.cast_sender().start_playback().await?;
                    }
                    SwDeviceKind::Dlna => {
                        info!("PLAYER: Starting DLNA playback - stopping local audio");
                        // Load media and start playback on DLNA device immediately
                        if let Some(station) = self.station() {
                            if let Some(url) = station.stream_url() {
                                if let Err(e) = self.dlna_sender()
                                    .load_media(
                                        url.as_ref(),
                                        &station
                                            .metadata()
                                            .favicon
                                            .map(|u| u.to_string())
                                            .unwrap_or_default(),
                                        &station.title(),
                                    )
                                {
                                    error!("PLAYER: Failed to load DLNA media: {}", e);
                                    return Err(e);
                                }
                                
                                // Start DLNA playback and set state to Playing
                                if let Err(e) = self.dlna_sender().start_playback() {
                                    error!("PLAYER: Failed to start DLNA playback: {}", e);
                                    return Err(e);
                                }
                                
                                // Manually set player state to Playing since we skip GStreamer
                                info!("PLAYER: Setting DLNA playback state to Playing");
                                if let Some(sender) = self.imp().gst_sender.get() {
                                    let _ = sender.send_blocking(GstreamerChange::PlaybackState(SwPlaybackState::Playing));
                                }
                            }
                        }
                    }
                }
            }
        }

        result
    }

    pub async fn disconnect_device(&self) {
        if let Some(device) = self.device() {
            #[cfg(feature = "dlna-debug")]
            {
                println!("🟡 DISCONNECT: === DEVICE DISCONNECT REQUESTED ===");
                println!("🟡 DISCONNECT: Device type: {:?}", device.kind());
            }
            info!("PLAYER: Disconnecting device: {:?}", device.kind());
            
            // Stop playback on the device first
            match device.kind() {
                SwDeviceKind::Cast => {
                    #[cfg(feature = "dlna-debug")]
                    println!("🟡 DISCONNECT: Stopping Cast playback");
                    info!("PLAYER: Stopping Cast playback");
                    self.cast_sender()
                        .stop_playback()
                        .await
                        .handle_error("Unable to stop Google Cast playback");
                    
                    #[cfg(feature = "dlna-debug")]
                    println!("🟡 DISCONNECT: Disconnecting Cast device");
                    info!("PLAYER: Disconnecting Cast device");
                    self.cast_sender().disconnect().await;
                }
                SwDeviceKind::Dlna => {
                    #[cfg(feature = "dlna-debug")]
                    {
                        println!("🟡 DISCONNECT: DLNA device - stopping playback and FFmpeg");
                        println!("🟡 DISCONNECT: Calling dlna_sender().stop_playback()");
                    }
                    info!("PLAYER: Stopping DLNA playback and FFmpeg proxy");
                    
                    if let Err(e) = self.dlna_sender().stop_playback() {
                        #[cfg(feature = "dlna-debug")]
                        println!("🟡 DISCONNECT: ❌ Failed to stop DLNA playback: {}", e);
                        warn!("PLAYER: Failed to stop DLNA playback: {}", e);
                    } else {
                        #[cfg(feature = "dlna-debug")]
                        println!("🟡 DISCONNECT: ✅ DLNA playback stopped");
                    }
                    
                    #[cfg(feature = "dlna-debug")]
                    println!("🟡 DISCONNECT: Calling dlna_sender().disconnect()");
                    info!("PLAYER: Disconnecting DLNA device");
                    self.dlna_sender().disconnect();
                    #[cfg(feature = "dlna-debug")]
                    println!("🟡 DISCONNECT: ✅ DLNA device disconnected");
                }
            };

            // Stop any ongoing device discovery to prevent scans in local mode
            #[cfg(feature = "dlna-debug")]
            println!("🟡 DISCONNECT: Stopping device discovery");
            info!("PLAYER: Stopping device discovery scan");
            self.device_discovery().stop();

            // Clear the device reference FIRST to prevent compatibility checks during disconnection
            #[cfg(feature = "dlna-debug")]
            println!("🟡 DISCONNECT: Clearing device reference");
            *self.imp().device.borrow_mut() = None;
            
            // Force immediate notification to ensure UI updates happen before any other operations
            self.notify_has_device();
            self.notify_device();

            // Reset player state to Stopped to allow local playback
            #[cfg(feature = "dlna-debug")]
            println!("🟡 DISCONNECT: Resetting player state to Stopped");
            info!("PLAYER: Resetting player state for local playback");
            if let Some(sender) = self.imp().gst_sender.get() {
                let _ = sender.send_blocking(GstreamerChange::PlaybackState(SwPlaybackState::Stopped));
            }

            // Clear any failure state that might have been set during device playback
            #[cfg(feature = "dlna-debug")]
            println!("🟡 DISCONNECT: Clearing any failure state");
            if let Some(sender) = self.imp().gst_sender.get() {
                let _ = sender.send_blocking(GstreamerChange::Failure(String::new()));
            }

            // Restore previous gstreamer volume for local playback
            let volume = {
                let backend = self.imp().backend.get().unwrap().borrow_mut();
                backend.set_mute(false);
                backend.volume()
            };
            debug!("Restore previous volume: {}", volume);
            self.set_volume(volume);
            
            // Ensure UI state is fully reset by notifying all relevant properties
            self.notify_state();
            self.notify_has_station();
            
            #[cfg(feature = "dlna-debug")]
            println!("🟡 DISCONNECT: ✅ Device disconnected - UI and state reset to local playback");
            info!("PLAYER: Device disconnected - ready for local playback");
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
