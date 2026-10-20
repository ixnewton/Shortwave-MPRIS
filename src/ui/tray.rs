// Shortwave - tray.rs
// Copyright (C) 2025  Felix Häcker <haeckerfelix@gnome.org>
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

use glib::clone;
use gtk::{glib, prelude::*};
use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;

use crate::app::SwApplication;
use crate::audio::SwPlaybackState;
use crate::config;
use crate::i18n::i18n;
use crate::utils;

/// System tray icon (StatusNotifierItem) shown while the application is running.
///
/// Left click toggles between foreground (window shown) and background mode
/// (window closed, playback continues). The context menu offers show/hide,
/// play/pause and a full application exit including any active ffmpeg proxy.
#[derive(Debug, Default)]
pub struct SwTray {
    window_visible: bool,
    playing: bool,
    status_message: String,
}

impl ksni::Tray for SwTray {
    fn id(&self) -> String {
        config::APP_ID.to_string()
    }

    fn title(&self) -> String {
        config::NAME.to_string()
    }

    fn icon_name(&self) -> String {
        // Use the symbolic icon variant so the tray icon is line-drawn and
        // follows the system theme / panel foreground color
        format!("{}-symbolic", config::APP_ID)
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: config::APP_ID.to_string(),
            title: config::NAME.to_string(),
            description: self.status_message.clone(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: if self.window_visible {
                    i18n("Hide Window")
                } else {
                    i18n("Show Window")
                },
                activate: Box::new(|_tray: &mut Self| Self::toggle_window()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: if self.playing {
                    i18n("Pause")
                } else {
                    i18n("Play")
                },
                activate: Box::new(|_tray: &mut Self| Self::toggle_playback()),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: i18n("Exit Application"),
                activate: Box::new(|_tray: &mut Self| Self::exit_application()),
                ..Default::default()
            }
            .into(),
        ]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        Self::toggle_window();
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        Self::toggle_window();
    }
}

impl SwTray {
    pub async fn start() -> Result<SwTrayHandle, ksni::Error> {
        let sandboxed = ashpd::is_sandboxed().await;

        let handle = SwTray::default()
            .disable_dbus_name(sandboxed)
            .assume_sni_available(true)
            .spawn()?;

        let handle = SwTrayHandle(handle);
        handle.sync_playback();

        // The window may already be visible at this point, as the tray is
        // started asynchronously after the window was presented
        let visible = SwApplication::default()
            .active_window()
            .map(|w| w.is_visible())
            .unwrap_or(false);
        handle.sync_window_visible(visible);
        Ok(handle)
    }

    /// Connect application signals that keep the tray state in sync.
    /// Must be called once on the main thread.
    pub fn connect_signals(app: &SwApplication) {
        let player = app.player();
        player.connect_state_notify(clone!(
            #[weak]
            app,
            move |_| {
                if let Some(handle) = app.tray_handle() {
                    handle.sync_playback();
                }
            }
        ));
        player.connect_station_notify(clone!(
            #[weak]
            app,
            move |_| {
                if let Some(handle) = app.tray_handle() {
                    handle.sync_playback();
                }
            }
        ));

        for window in app.windows() {
            Self::watch_window(&window, app);
        }
        app.connect_window_added(clone!(
            #[weak]
            app,
            move |_, window| {
                Self::watch_window(window, &app);
            }
        ));
    }

    fn watch_window(window: &gtk::Window, app: &SwApplication) {
        window.connect_notify_local(Some("visible"), clone!(
            #[weak]
            app,
            move |win, _| {
                if let Some(handle) = app.tray_handle() {
                    handle.sync_window_visible(win.is_visible());
                }
            }
        ));
    }

    fn on_main(f: impl FnOnce() + Send + 'static) {
        glib::MainContext::default().invoke(f);
    }

    fn toggle_window() {
        Self::on_main(|| {
            let app = SwApplication::default();
            let window = app.application_window();

            if window.is_visible() {
                // Goes through the regular close request path, so background
                // playback portal permissions are verified as usual
                window.close();
            } else {
                app.activate();
            }
        });
    }

    fn toggle_playback() {
        Self::on_main(|| {
            glib::spawn_future_local(async move {
                SwApplication::default().player().toggle_playback().await;
            });
        });
    }

    fn exit_application() {
        Self::on_main(|| {
            let app = SwApplication::default();
            glib::spawn_future_local(async move {
                // Stop playback first, this also terminates any active ffmpeg
                // proxy process for Cast / DLNA devices
                app.player().stop_playback().await;
                app.quit();
            });
        });
    }
}

#[derive(Clone)]
pub struct SwTrayHandle(ksni::blocking::Handle<SwTray>);

impl SwTrayHandle {
    pub fn sync_playback(&self) {
        let player = SwApplication::default().player();
        let playing = player.state() == SwPlaybackState::Playing;
        let message = utils::playback_status_message(&player);

        self.0.update(move |tray| {
            tray.playing = playing;
            tray.status_message = message;
        });
    }

    pub fn sync_window_visible(&self, visible: bool) {
        self.0.update(move |tray| tray.window_visible = visible);
    }

    pub fn shutdown(&self) {
        self.0.shutdown();
    }
}
