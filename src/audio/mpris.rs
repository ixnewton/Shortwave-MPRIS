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
use gtk::{glib, prelude::ApplicationExt};
use mpris_server::{zbus::Result, Metadata, PlaybackStatus, Player};

use super::SwPlaybackState;
use crate::{app::SwApplication, config};

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
            .can_pause(false)
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
            SwPlaybackState::Stopped => PlaybackStatus::Stopped,
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
