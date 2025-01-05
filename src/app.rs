// Shortwave - app.rs
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

use std::cell::{Cell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gio::subclass::prelude::ApplicationImpl;
use glib::{clone, Properties};
use gtk::glib::VariantTy;
use gtk::{gio, glib};

use crate::api::client;
use crate::api::CoverLoader;
use crate::audio::{SwPlaybackState, SwPlayer, SwRecordingState};
use crate::config;
use crate::database::SwLibrary;
use crate::i18n::i18n;
use crate::settings::*;
use crate::ui::{SwApplicationWindow, SwTrackDialog};

mod imp {
    use super::*;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::SwApplication)]
    pub struct SwApplication {
        #[property(get)]
        library: SwLibrary,
        #[property(get)]
        player: SwPlayer,
        #[property(get)]
        rb_server: RefCell<Option<String>>,
        #[property(get, set=Self::set_background_playback)]
        background_playback: Cell<bool>,

        pub cover_loader: CoverLoader,
        pub inhibit_cookie: Cell<u32>,
        pub background_hold: RefCell<Option<gio::ApplicationHoldGuard>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwApplication {
        const NAME: &'static str = "SwApplication";
        type ParentType = adw::Application;
        type Type = super::SwApplication;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwApplication {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            obj.add_action_entries([
                // app.show-track
                gio::ActionEntry::builder("show-track")
                    .parameter_type(Some(VariantTy::STRING))
                    .activate(move |app: &super::SwApplication, _, uuid| {
                        app.activate();

                        let uuid = uuid.and_then(|v| v.str()).unwrap_or_default();
                        let window = app.application_window();

                        if let Some(track) = app.player().track_by_uuid(uuid) {
                            SwTrackDialog::new(&track).present(Some(&window));
                        } else {
                            window.show_notification(&i18n("Track no longer available"));
                        }
                    })
                    .build(),
                // app.save-track
                gio::ActionEntry::builder("save-track")
                    .parameter_type(Some(VariantTy::STRING))
                    .activate(move |app: &super::SwApplication, _, uuid| {
                        app.activate();

                        let uuid = uuid.and_then(|v| v.str()).unwrap_or_default();
                        let window = app.application_window();

                        // Check if track uuid matches current playing track uuid
                        if let Some(track) = app.player().playing_track() {
                            if track.uuid() == uuid && track.state() == SwRecordingState::Recording
                            {
                                track.set_save_when_recorded(true);
                                SwTrackDialog::new(&track).present(Some(&window));
                                return;
                            }
                        }

                        window.show_notification(&i18n("This track is currently not recorded"));
                    })
                    .build(),
                // app.cancel-recording
                gio::ActionEntry::builder("cancel-recording")
                    .parameter_type(Some(VariantTy::STRING))
                    .activate(move |app: &super::SwApplication, _, uuid| {
                        app.activate();

                        let window: SwApplicationWindow = app.application_window();
                        let uuid = uuid.and_then(|v| v.str()).unwrap_or_default();

                        // Check if track uuid matches current playing track uuid
                        if let Some(track) = app.player().playing_track() {
                            if track.uuid() == uuid && track.state() == SwRecordingState::Recording
                            {
                                app.player().cancel_recording();
                                SwTrackDialog::new(&track).present(Some(&window));
                                return;
                            }
                        }

                        window.show_notification(&i18n("This track is currently not recorded"));
                    })
                    .build(),
                // app.quit
                gio::ActionEntry::builder("quit")
                    .activate(move |app: &super::SwApplication, _, _| {
                        app.quit();
                    })
                    .build(),
            ]);

            obj.set_accels_for_action("win.show-preferences", &["<primary>comma"]);
            obj.set_accels_for_action("app.quit", &["<primary>q"]);
            obj.set_accels_for_action("window.close", &["<primary>w"]);
            obj.set_accels_for_action("player.toggle-playback", &["<primary>space"]);
        }
    }

    impl ApplicationImpl for SwApplication {
        fn startup(&self) {
            self.parent_startup();

            // Find radiobrowser server and update library data
            let fut = clone!(
                #[weak(rename_to = imp)]
                self,
                async move {
                    imp.lookup_rb_server().await;
                }
            );
            glib::spawn_future_local(fut);

            // Restore previously played station / volume
            self.player.restore_state();

            settings_manager::bind_property(
                Key::BackgroundPlayback,
                &*self.obj(),
                "background-playback",
            );
        }

        fn activate(&self) {
            self.parent_activate();

            debug!("gio::Application -> activate()");
            self.obj().application_window().present();
        }

        fn shutdown(&self) {
            self.parent_shutdown();
            debug!("gio::Application -> shutdown()");

            glib::spawn_future_local(async {
                super::SwApplication::default()
                    .cover_loader()
                    .prune_cache()
                    .await;
            });
        }
    }

    impl GtkApplicationImpl for SwApplication {
        fn window_removed(&self, window: &gtk::Window) {
            self.parent_window_removed(window);
            let obj = self.obj();

            if obj.active_window().is_none()
                && obj.background_playback()
                && obj.player().state() != SwPlaybackState::Playing
            {
                debug!("No active playback, quit application.");
                obj.quit();
            }
        }
    }

    impl AdwApplicationImpl for SwApplication {}

    impl SwApplication {
        fn set_background_playback(&self, enabled: bool) {
            dbg!(&enabled);
            self.background_playback.set(enabled);

            if enabled {
                self.background_hold.replace(Some(self.obj().hold()));
            } else {
                self.background_hold.replace(None);
            }
        }

        async fn lookup_rb_server(&self) {
            // Try to find a working radio-browser server
            let rb_server = client::lookup_rb_server().await;

            self.rb_server.borrow_mut().clone_from(&rb_server);
            self.obj().notify("rb-server");

            if let Some(rb_server) = &rb_server {
                info!("Using radio-browser.info REST api: {rb_server}");
                // Refresh library data
                let _ = self.library.update_data().await;
            } else {
                warn!("Unable to find radio-browser.info server.");
            }
        }
    }
}

glib::wrapper! {
    pub struct SwApplication(ObjectSubclass<imp::SwApplication>)
        @extends gio::Application, gtk::Application, adw::Application,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl SwApplication {
    pub fn run() -> glib::ExitCode {
        debug!(
            "{} ({}) ({}) - Version {} ({})",
            config::NAME,
            config::APP_ID,
            config::VCS_TAG,
            config::VERSION,
            config::PROFILE
        );
        info!("Isahc version: {}", isahc::version());

        // Create new GObject and downcast it into SwApplication
        let app = glib::Object::builder::<SwApplication>()
            .property("application-id", Some(config::APP_ID))
            .property("flags", gio::ApplicationFlags::empty())
            .property("resource-base-path", Some(config::PATH_ID))
            .build();

        // Start running gtk::Application
        app.run()
    }

    pub fn application_window(&self) -> SwApplicationWindow {
        if let Some(window) = self.active_window() {
            window.downcast::<SwApplicationWindow>().unwrap()
        } else {
            let window = SwApplicationWindow::new();
            self.add_window(&window);

            info!("Created application window.");
            window
        }
    }

    pub fn cover_loader(&self) -> CoverLoader {
        self.imp().cover_loader.clone()
    }

    pub fn set_inhibit(&self, inhibit: bool) {
        let imp = self.imp();

        if inhibit && imp.inhibit_cookie.get() == 0 {
            debug!("Install inhibitor");

            let cookie = self.inhibit(
                Some(&self.application_window()),
                gtk::ApplicationInhibitFlags::SUSPEND,
                Some(&i18n("Active Playback")),
            );
            imp.inhibit_cookie.set(cookie);
        } else if imp.inhibit_cookie.get() != 0 {
            debug!("Remove inhibitor");

            self.uninhibit(imp.inhibit_cookie.get());
            imp.inhibit_cookie.set(0);
        }
    }
}

impl Default for SwApplication {
    fn default() -> Self {
        gio::Application::default()
            .expect("Could not get default GApplication")
            .downcast()
            .unwrap()
    }
}
