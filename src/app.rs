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
use std::rc::Rc;
use std::str::FromStr;

use adw::prelude::*;
use adw::subclass::prelude::*;
use async_channel::{Receiver, Sender};
use gio::subclass::prelude::ApplicationImpl;
use glib::{clone, Properties};
use gtk::glib::WeakRef;
use gtk::{gio, glib};

use crate::api::SwClient;
use crate::audio::SwPlayer;
use crate::audio::{GCastDevice, PlaybackState, Player, Song};
use crate::config;
use crate::database::SwLibrary;
use crate::model::SwSorting;
use crate::settings::{settings_manager, Key, SwSettingsDialog};
use crate::ui::{about_dialog, SwApplicationWindow};

#[derive(Debug, Clone)]
pub enum Action {
    // Audio Playback
    PlaybackConnectGCastDevice(GCastDevice),
    PlaybackDisconnectGCastDevice,
    PlaybackSet(bool),
    PlaybackToggle,
    PlaybackSaveSong(Song),

    SettingsKeyChanged(Key),
}

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

        pub sender: Sender<Action>,
        pub receiver: RefCell<Option<Receiver<Action>>>,

        pub window: OnceCell<WeakRef<SwApplicationWindow>>,
        pub legacy_player: Rc<Player>,

        pub settings: gio::Settings,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwApplication {
        const NAME: &'static str = "SwApplication";
        type ParentType = adw::Application;
        type Type = super::SwApplication;

        fn new() -> Self {
            let (sender, r) = async_channel::bounded(10);
            let receiver = RefCell::new(Some(r));

            let library = SwLibrary::default();
            let player = SwPlayer::default();
            let rb_server = RefCell::default();

            let window = OnceCell::new();
            let legacy_player = Player::new(sender.clone());

            let settings = settings_manager::settings();

            Self {
                library,
                player,
                rb_server,
                sender,
                receiver,
                window,
                legacy_player,
                settings,
            }
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwApplication {}

    impl ApplicationImpl for SwApplication {
        fn activate(&self) {
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

            // Setup app level GActions
            app.setup_gactions();

            // Setup action channel
            let receiver = self.receiver.borrow_mut().take().unwrap();
            glib::spawn_future_local(clone!(
                #[weak]
                app,
                async move {
                    while let Ok(action) = receiver.recv().await {
                        app.process_action(action);
                    }
                }
            ));

            // Find radiobrowser server and update library data
            let fut = clone!(
                #[weak]
                app,
                async move {
                    app.lookup_rb_server().await;
                }
            );
            glib::spawn_future_local(fut);

            // Setup settings signal (we get notified when a key gets changed)
            self.settings.connect_changed(
                None,
                clone!(
                    #[strong(rename_to = sender)]
                    self.sender,
                    move |_, key_str| {
                        let key: Key = Key::from_str(key_str).unwrap();
                        crate::utils::send(&sender, Action::SettingsKeyChanged(key));
                    }
                ),
            );

            // Small workaround to update every view to the correct sorting/order.
            crate::utils::send(&self.sender, Action::SettingsKeyChanged(Key::ViewSorting));
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

    fn create_window(&self) -> SwApplicationWindow {
        let window = SwApplicationWindow::new();
        self.add_window(&window);

        window.present();
        window
    }

    fn setup_gactions(&self) {
        let window = SwApplicationWindow::default();

        self.add_action_entries([
            // app.show-preferences
            gio::ActionEntry::builder("show-preferences")
                .activate(clone!(
                    #[weak]
                    window,
                    move |_, _, _| {
                        let settings_window = SwSettingsDialog::default();
                        settings_window.present(Some(&window));
                    }
                ))
                .build(),
            // app.quit
            gio::ActionEntry::builder("quit")
                .activate(clone!(
                    #[weak]
                    window,
                    move |_, _, _| {
                        window.close();
                    }
                ))
                .build(),
            // app.about
            gio::ActionEntry::builder("about")
                .activate(clone!(
                    #[weak]
                    window,
                    move |_, _, _| {
                        about_dialog::show(&window);
                    }
                ))
                .build(),
        ]);
        self.set_accels_for_action("app.show-preferences", &["<primary>comma"]);
        self.set_accels_for_action("app.quit", &["<primary>q"]);
        self.set_accels_for_action("window.close", &["<primary>w"]);
    }

    fn process_action(&self, action: Action) -> glib::ControlFlow {
        let imp = self.imp();
        if self.active_window().is_none() {
            return glib::ControlFlow::Continue;
        }

        match action {
            Action::PlaybackConnectGCastDevice(device) => {
                imp.legacy_player.connect_to_gcast_device(device)
            }
            Action::PlaybackDisconnectGCastDevice => {
                imp.legacy_player.disconnect_from_gcast_device()
            }
            Action::PlaybackSet(true) => imp.legacy_player.set_playback(PlaybackState::Playing),
            Action::PlaybackSet(false) => imp.legacy_player.set_playback(PlaybackState::Stopped),
            Action::PlaybackToggle => imp.legacy_player.toggle_playback(),
            Action::PlaybackSaveSong(song) => imp.legacy_player.save_song(song),
            Action::SettingsKeyChanged(key) => self.apply_settings_changes(key),
        }
        glib::ControlFlow::Continue
    }

    fn apply_settings_changes(&self, key: Key) {
        debug!("Settings key changed: {:?}", &key);
        match key {
            Key::ViewSorting | Key::ViewOrder => {
                let value = settings_manager::string(Key::ViewSorting);
                let sorting = SwSorting::from_str(&value).unwrap();
                let order = settings_manager::string(Key::ViewOrder);
                let descending = order == "Descending";

                self.imp()
                    .window
                    .get()
                    .unwrap()
                    .upgrade()
                    .unwrap()
                    .set_sorting(sorting, descending);
            }
            _ => (),
        }
    }

    async fn lookup_rb_server(&self) {
        let imp = self.imp();

        // Try to find a working radio-browser server
        let rb_server = SwClient::lookup_rb_server().await;

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
