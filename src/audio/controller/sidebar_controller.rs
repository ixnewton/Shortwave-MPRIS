// Shortwave - sidebar_controller.rs
// Copyright (C) 2021-2023  Felix Häcker <haeckerfelix@gnome.org>
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
use std::rc::Rc;

use adw::prelude::*;
use async_channel::Sender;
use futures_util::future::FutureExt;
use glib::clone;
use gtk::{gio, glib};

use crate::api::{FaviconDownloader, SwStation};
use crate::app::{Action, SwApplication};
use crate::audio::{Controller, PlaybackState};
use crate::ui::{FaviconSize, StationFavicon, SwStationDialog};

pub struct SidebarController {
    pub widget: gtk::Box,
    sender: Sender<Action>,
    station: Rc<RefCell<Option<SwStation>>>,

    station_favicon: Rc<StationFavicon>,
    title_label: gtk::Label,
    subtitle_label: gtk::Label,
    subtitle_revealer: gtk::Revealer,
    action_revealer: gtk::Revealer,
    playback_button_stack: gtk::Stack,
    start_playback_button: gtk::Button,
    stop_playback_button: gtk::Button,
    loading_button: gtk::Button,
    error_label: gtk::Label,
    volume_button: gtk::ScaleButton,
    volume_signal_id: glib::signal::SignalHandlerId,

    action_group: gio::SimpleActionGroup,
}

impl SidebarController {
    pub fn new(sender: Sender<Action>) -> Self {
        let builder =
            gtk::Builder::from_resource("/de/haeckerfelix/Shortwave/gtk/sidebar_controller.ui");
        get_widget!(builder, gtk::Box, sidebar_controller);
        get_widget!(builder, gtk::Label, title_label);
        get_widget!(builder, gtk::Label, subtitle_label);
        get_widget!(builder, gtk::Revealer, subtitle_revealer);
        get_widget!(builder, gtk::Revealer, action_revealer);
        get_widget!(builder, gtk::Stack, playback_button_stack);
        get_widget!(builder, gtk::Button, start_playback_button);
        get_widget!(builder, gtk::Button, stop_playback_button);
        get_widget!(builder, gtk::Button, loading_button);
        get_widget!(builder, gtk::Label, error_label);
        get_widget!(builder, gtk::ScaleButton, volume_button);

        let station = Rc::new(RefCell::new(None));

        get_widget!(builder, gtk::Box, favicon_box);
        let station_favicon = Rc::new(StationFavicon::new(FaviconSize::Big));
        favicon_box.append(&station_favicon.widget);

        // volume_button | We need the volume_signal_id later to block the signal
        let volume_signal_id = volume_button.connect_value_changed(clone!(
            #[strong]
            sender,
            move |_, value| {
                SwApplication::default().player().set_volume(value);
            }
        ));

        // action group
        let action_group = gio::SimpleActionGroup::new();
        sidebar_controller.insert_action_group("player", Some(&action_group));

        let controller = Self {
            widget: sidebar_controller,
            sender,
            station,
            station_favicon,
            title_label,
            subtitle_label,
            action_revealer,
            subtitle_revealer,
            playback_button_stack,
            start_playback_button,
            stop_playback_button,
            loading_button,
            error_label,
            volume_button,
            volume_signal_id,
            action_group,
        };

        controller.setup_signals();
        controller
    }

    fn setup_signals(&self) {
        // start_playback_button
        self.start_playback_button.connect_clicked(clone!(
            #[strong(rename_to = sender)]
            self.sender,
            move |_| {
                crate::utils::send(&sender, Action::PlaybackSet(true));
            }
        ));

        // stop_playback_button
        self.stop_playback_button.connect_clicked(clone!(
            #[strong(rename_to = sender)]
            self.sender,
            move |_| {
                crate::utils::send(&sender, Action::PlaybackSet(false));
            }
        ));

        // stop_playback_button
        self.loading_button.connect_clicked(clone!(
            #[strong(rename_to = sender)]
            self.sender,
            move |_| {
                crate::utils::send(&sender, Action::PlaybackSet(false));
            }
        ));

        // details button
        self.action_group
            .add_action_entries([gio::ActionEntry::builder("show-details")
                .activate(clone!(
                    #[strong(rename_to = station)]
                    self.station,
                    #[weak(rename_to = widget)]
                    self.widget,
                    move |_, _, _| {
                        let station = station.borrow().clone().unwrap();
                        let station_dialog = SwStationDialog::new(&station);
                        station_dialog.present(Some(&widget));
                    }
                ))
                .build()]);
    }
}

impl Controller for SidebarController {
    fn set_station(&self, station: SwStation) {
        self.action_revealer.set_reveal_child(true);
        self.title_label.set_text(&station.metadata().name);
        self.title_label
            .set_tooltip_text(Some(station.metadata().name.as_str()));
        *self.station.borrow_mut() = Some(station.clone());

        // Download & set icon

        let station_favicon = self.station_favicon.clone();

        if let Some(texture) = station.favicon() {
            station_favicon.set_paintable(&texture.upcast());
        } else if let Some(favicon) = station.metadata().favicon {
            let fut = FaviconDownloader::download(favicon).map(move |paintable| match paintable {
                Ok(paintable) => station_favicon.set_paintable(&paintable),
                Err(error) => {
                    debug!("Could not load favicon: {}", error);
                    station_favicon.reset()
                }
            });
            glib::spawn_future_local(fut);
        } else {
            self.station_favicon.reset();
        }

        // reset everything else
        self.error_label.set_text(" ");
        self.subtitle_revealer.set_reveal_child(false);
    }

    fn set_playback_state(&self, playback_state: &PlaybackState) {
        let child_name = match playback_state {
            PlaybackState::Playing => "stop_playback",
            PlaybackState::Stopped => "start_playback",
            PlaybackState::Loading => "loading",
            PlaybackState::Failure(msg) => {
                let mut text = self.error_label.text().to_string();
                text = text + " " + msg;
                self.error_label.set_text(&text);
                "error"
            }
        };
        self.playback_button_stack
            .set_visible_child_name(child_name);
    }

    fn set_volume(&self, volume: f64) {
        // We need to block the signal, otherwise we risk creating a endless loop
        glib::signal::signal_handler_block(&self.volume_button, &self.volume_signal_id);
        self.volume_button.set_value(volume);
        glib::signal::signal_handler_unblock(&self.volume_button, &self.volume_signal_id);
    }

    fn set_song_title(&self, title: &str) {
        if !title.is_empty() {
            self.subtitle_label.set_text(title);
            self.subtitle_label.set_tooltip_text(Some(title));
            self.subtitle_revealer.set_reveal_child(true);
        } else {
            self.subtitle_label.set_text("");
            self.subtitle_label.set_tooltip_text(None);
            self.subtitle_revealer.set_reveal_child(false);
        }
    }
}
