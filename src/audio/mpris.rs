// Shortwave - mpris.rs
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

use std::rc::Rc;

use glib::clone;
use gtk::{
    glib,
    prelude::{ApplicationExt, GtkApplicationExt, WidgetExt},
};
use mpris_server::{zbus::Result, Metadata, PlaybackStatus, Player};

use crate::app::SwApplication;
use crate::audio::playback_state::SwPlaybackState;
use crate::config;
use crate::utils;

#[derive(Debug, Clone)]
pub struct MprisServer {
    player: Rc<Player>,
}

impl MprisServer {
    pub async fn start() -> Result<Self> {
        let player = Player::builder(config::APP_ID)
            .desktop_entry(config::APP_ID)
            .identity(config::NAME)
            .can_play(true)
            // This is not true, but MPRIS has no concept of play/stop
            // for live streams, so we only can use play/pause here
            .can_pause(true)
            .can_go_next(true)
            .can_go_previous(true)
            .can_seek(false)
            .can_set_fullscreen(false)
            .can_raise(true)
            .can_quit(true)
            .build()
            .await?;

        let server = Self {
            player: Rc::new(player),
        };
        let player = SwApplication::default().player();

        // Shortwave side callbacks
        player.connect_state_notify(clone!(
            #[strong]
            server,
            move |_| {
                glib::spawn_future_local(clone!(
                    #[strong]
                    server,
                    async move {
                        server.update_mpris_plaback_status().await;
                    }
                ));
            }
        ));

        player.connect_station_notify(clone!(
            #[strong]
            server,
            move |_| {
                glib::spawn_future_local(clone!(
                    #[strong]
                    server,
                    async move {
                        server.update_mpris_metadata().await;
                    }
                ));
            }
        ));

        player.connect_playing_track_notify(clone!(
            #[strong]
            server,
            move |_| {
                glib::spawn_future_local(clone!(
                    #[strong]
                    server,
                    async move {
                        server.update_mpris_metadata().await;
                    }
                ));
            }
        ));

        player.connect_volume_notify(clone!(
            #[strong]
            server,
            move |_| {
                glib::spawn_future_local(clone!(
                    #[strong]
                    server,
                    async move {
                        server.update_mpris_volume().await;
                    }
                ));
            }
        ));

        // Mpris side callbacks
        server.player.connect_play_pause(|_| {
            glib::spawn_future_local(async move {
                SwApplication::default().player().toggle_playback().await;
            });
        });

        server.player.connect_play(|_| {
            glib::spawn_future_local(async move {
                SwApplication::default().player().start_playback().await;
            });
        });

        server.player.connect_pause(|_| {
            glib::spawn_future_local(async move {
                SwApplication::default().player().stop_playback().await;
            });
        });

        server.player.connect_stop(|_| {
            glib::spawn_future_local(async move {
                SwApplication::default().player().stop_playback().await;
            });
        });

        server.player.connect_set_volume(|_, volume| {
            SwApplication::default().player().set_volume(volume);
        });

        server.player.connect_raise(|_| {
            SwApplication::default().activate();
        });

        server.player.connect_quit(|_| {
            SwApplication::default().quit();
        });

        // Add handlers for next/previous track in favorites
        server.player.connect_next(|_| {
            glib::spawn_future_local(async move {
                let app = SwApplication::default();
                let library = app.library();
                let player = app.player();

                if app.background_playback()
                    && utils::OptionExt::is_none_or(&app.active_window(), |w| !w.is_visible())
                    && !utils::background_portal_permissions().await
                {
                    debug!("No background portal permissions for next command");
                } else if let Some(next_station) = library.get_next_favorite() {
                    let was_playing = matches!(player.state(), SwPlaybackState::Playing);
                    player.set_station(next_station).await;
                    if was_playing {
                        player.start_playback().await;
                    }
                }
            });
        });

        server.player.connect_previous(|_| {
            glib::spawn_future_local(async move {
                let app = SwApplication::default();
                let library = app.library();
                let player = app.player();

                if app.background_playback()
                    && utils::OptionExt::is_none_or(&app.active_window(), |w| !w.is_visible())
                    && !utils::background_portal_permissions().await
                {
                    debug!("No background portal permissions for previous command");
                } else if let Some(prev_station) = library.get_previous_favorite() {
                    let was_playing = matches!(player.state(), SwPlaybackState::Playing);
                    player.set_station(prev_station).await;
                    if was_playing {
                        player.start_playback().await;
                    }
                }
            });
        });

        glib::spawn_future_local(server.player.run());
        server.update_mpris_plaback_status().await;
        server.update_mpris_metadata().await;
        server.update_mpris_volume().await;

        Ok(server)
    }

    async fn update_mpris_metadata(&self) {
        let player = SwApplication::default().player();
        let mut metadata = Metadata::builder();

        if let Some(track) = player.playing_track() {
            metadata = metadata.title(track.title());
        }

        if let Some(station) = player.station() {
            metadata = metadata.artist(vec![station.title()]);

            // TODO: Add support for caching / local stations
            if let Some(url) = station.metadata().favicon {
                metadata = metadata.art_url(url);
            }

            if let Some(url) = station.stream_url() {
                metadata = metadata.url(url);
            }
        }

        if let Err(err) = self.player.set_metadata(metadata.build()).await {
            error!("Unable to update mpris metadata: {:?}", err.to_string())
        }
    }

    async fn update_mpris_plaback_status(&self) {
        let player = SwApplication::default().player();

        let can_play = player.has_station();
        if let Err(err) = self.player.set_can_play(can_play).await {
            error!("Unable to update mpris can-play: {:?}", err.to_string())
        }

        let playback_status = match player.state() {
            SwPlaybackState::Stopped => PlaybackStatus::Paused, // Map Stopped to Paused for MPRIS
            SwPlaybackState::Playing => PlaybackStatus::Playing,
            SwPlaybackState::Loading => PlaybackStatus::Playing,
            SwPlaybackState::Failure => PlaybackStatus::Stopped,
        };

        if let Err(err) = self.player.set_playback_status(playback_status).await {
            error!(
                "Unable to update mpris playback status: {:?}",
                err.to_string()
            )
        }
    }

    async fn update_mpris_volume(&self) {
        let player = SwApplication::default().player();
        if let Err(err) = self.player.set_volume(player.volume()).await {
            error!("Unable to update mpris volume: {:?}", err.to_string())
        }
    }
}
