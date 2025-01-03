// Shortwave - app.rs
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

use std::cell::{OnceCell, RefCell};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gio::subclass::prelude::ApplicationImpl;
use glib::{clone, Properties};
use gtk::glib::{VariantTy, WeakRef};
use gtk::{gio, glib};

use crate::api::client;
use crate::api::CoverLoader;
use crate::audio::{SwPlayer, SwRecordingState};
use crate::config;
use crate::database::SwLibrary;
use crate::i18n::i18n;
use crate::settings::settings_manager;
use crate::ui::{about_dialog, SwApplicationWindow, SwPreferencesDialog, SwTrackDialog};

mod imp {
    use super::*;

    #[derive(Properties)]
    #[properties(wrapper_type = super::SwApplication)]
    pub struct SwApplication {
        #[property(get)]
        pub library: SwLibrary,
        #[property(get)]
        pub player: SwPlayer,
        #[property(get)]
        pub rb_server: RefCell<Option<String>>,

        pub window: OnceCell<WeakRef<SwApplicationWindow>>,
        pub cover_loader: CoverLoader,
        pub settings: gio::Settings,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwApplication {
        const NAME: &'static str = "SwApplication";
        type ParentType = adw::Application;
        type Type = super::SwApplication;

        fn new() -> Self {
            let library = SwLibrary::default();
            let player = SwPlayer::new();
            let rb_server = RefCell::default();

            let window = OnceCell::new();
            let cover_loader = CoverLoader::new();
            let settings = settings_manager::settings();

            Self {
                library,
                player,
                rb_server,
                window,
                cover_loader,
                settings,
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwApplication {}

    impl ApplicationImpl for SwApplication {
        fn startup(&self) {
            self.parent_startup();
            let obj = self.obj();

            obj.add_action_entries([
                // app.show-track
                gio::ActionEntry::builder("show-track")
                    .parameter_type(Some(VariantTy::STRING))
                    .activate(move |app: &super::SwApplication, _, uuid| {
                        app.activate();

                        let uuid = uuid.and_then(|v| v.str()).unwrap_or_default();
                        let player = SwPlayer::default();
                        let window = SwApplicationWindow::default();

                        if let Some(track) = player.track_by_uuid(uuid) {
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
                        app.ensure_activated();

                        let uuid = uuid.and_then(|v| v.str()).unwrap_or_default();
                        let player = SwPlayer::default();
                        let window = SwApplicationWindow::default();

                        // Check if track uuid matches current playing track uuid
                        if let Some(track) = player.playing_track() {
                            if track.uuid() == uuid && track.state() == SwRecordingState::Recording
                            {
                                track.set_save_when_recorded(true);
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
                        app.ensure_activated();

                        let uuid = uuid.and_then(|v| v.str()).unwrap_or_default();
                        let player = SwPlayer::default();
                        let window = SwApplicationWindow::default();

                        // Check if track uuid matches current playing track uuid
                        if let Some(track) = player.playing_track() {
                            if track.uuid() == uuid && track.state() == SwRecordingState::Recording
                            {
                                player.cancel_recording();
                                return;
                            }
                        }

                        window.show_notification(&i18n("This track is currently not recorded"));
                    })
                    .build(),
                // app.show-preferences
                gio::ActionEntry::builder("show-preferences")
                    .activate(move |app: &super::SwApplication, _, _| {
                        app.activate();
                        let window = SwApplicationWindow::default();
                        let preferences_window = SwPreferencesDialog::default();
                        preferences_window.present(Some(&window));
                    })
                    .build(),
                // app.quit
                gio::ActionEntry::builder("quit")
                    .activate(move |app: &super::SwApplication, _, _| {
                        app.ensure_activated();
                        SwApplicationWindow::default().close();
                    })
                    .build(),
                // app.about
                gio::ActionEntry::builder("about")
                    .activate(move |app: &super::SwApplication, _, _| {
                        app.activate();
                        let window = SwApplicationWindow::default();
                        about_dialog::show(&window);
                    })
                    .build(),
            ]);

            obj.set_accels_for_action("app.show-preferences", &["<primary>comma"]);
            obj.set_accels_for_action("app.quit", &["<primary>q"]);
            obj.set_accels_for_action("window.close", &["<primary>w"]);
        }

        fn activate(&self) {
            self.parent_activate();

            debug!("gio::Application -> activate()");
            let app = self.obj();

            // If the window already exists,
            // present it instead creating a new one again.
            if let Some(weak_window) = self.window.get() {
                weak_window.upgrade().unwrap().present();
                info!("Application window presented.");
                return;
            }

            // No window available -> we have to create one
            let window = app.create_window();
            let _ = self.window.set(window.downgrade());
            info!("Created application window.");

            // Find radiobrowser server and update library data
            let fut = clone!(
                #[weak]
                app,
                async move {
                    app.lookup_rb_server().await;
                }
            );
            glib::spawn_future_local(fut);

            // Restore previously played station / volume
            self.player.restore_state();
        }

        fn shutdown(&self) {
            self.parent_shutdown();
            glib::spawn_future_local(async {
                super::SwApplication::default()
                    .cover_loader()
                    .prune_cache()
                    .await;
            });
        }
    }

    impl GtkApplicationImpl for SwApplication {}

    impl AdwApplicationImpl for SwApplication {}
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

    pub fn cover_loader(&self) -> CoverLoader {
        self.imp().cover_loader.clone()
    }

    fn create_window(&self) -> SwApplicationWindow {
        let window = SwApplicationWindow::new();
        self.add_window(&window);

        window.present();
        window
    }

    // Ensures that the app is activated, and the application window exists
    fn ensure_activated(&self) {
        if self.imp().window.get().is_none() {
            self.activate();
        }
    }

    async fn lookup_rb_server(&self) {
        let imp = self.imp();

        // Try to find a working radio-browser server
        let rb_server = client::lookup_rb_server().await;

        imp.rb_server.borrow_mut().clone_from(&rb_server);
        self.notify("rb-server");

        if let Some(rb_server) = &rb_server {
            info!("Using radio-browser.info REST api: {rb_server}");
            // Refresh library data
            let _ = imp.library.update_data().await;
        } else {
            warn!("Unable to find radio-browser.info server.");
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
