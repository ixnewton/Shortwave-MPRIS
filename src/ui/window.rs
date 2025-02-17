// Shortwave - window.rs
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

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use gtk::{gio, glib, CompositeTemplate};
use glib::subclass::InitializingObject;

use crate::app::SwApplication;
use crate::audio::SwPlaybackState;
use crate::config;
use crate::i18n::i18n;
use crate::settings::{settings_manager, Key};
use crate::ui::pages::{SwLibraryPage, SwSearchPage};
use crate::ui::player::{SwPlayerGadget, SwPlayerToolbar, SwPlayerView};
use crate::ui::{
    about_dialog, SwAddStationDialog, SwDeviceDialog, SwPreferencesDialog, SwStationDialog,
    ToastWindow,
};
use crate::utils;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/window.ui")]
    pub struct SwApplicationWindow {
        #[template_child]
        pub(super) library_page: TemplateChild<SwLibraryPage>,
        #[template_child]
        pub(super) search_page: TemplateChild<SwSearchPage>,

        #[template_child]
        pub(super) player_gadget: TemplateChild<SwPlayerGadget>,
        #[template_child]
        pub(super) player_toolbar: TemplateChild<SwPlayerToolbar>,
        #[template_child]
        pub(super) player_view: TemplateChild<SwPlayerView>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwApplicationWindow {
        const NAME: &'static str = "SwApplicationWindow";
        type ParentType = adw::ApplicationWindow;
        type Type = super::SwApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            // player
            klass.install_action_async("player.start-playback", None, |_, _, _| async move {
                SwApplication::default().player().start_playback().await;
            });
            klass.install_action_async("player.stop-playback", None, |_, _, _| async move {
                SwApplication::default().player().stop_playback().await;
            });
            klass.install_action_async("player.toggle-playback", None, |_, _, _| async move {
                SwApplication::default().player().toggle_playback().await;
            });
            klass.install_action("player.show-device-connect", None, move |win, _, _| {
                let is_visible = win
                    .visible_dialog()
                    .map(|d| d.downcast::<SwDeviceDialog>().is_ok())
                    .unwrap_or(false);

                if !is_visible {
                    SwDeviceDialog::new().present(Some(win));
                }
            });
            klass.install_action("player.show-station-details", None, move |win, _, _| {
                if let Some(station) = SwApplication::default().player().station() {
                    let is_visible = win
                        .visible_dialog()
                        .map(|d| d.downcast::<SwStationDialog>().is_ok())
                        .unwrap_or(false);

                    if !is_visible {
                        SwStationDialog::new(&station).present(Some(win));
                    }
                }
            });

            // win
            klass.install_action("win.open-radio-browser-info", None, move |win, _, _| {
                win.show_uri("https://www.radio-browser.info/");
            });
            klass.install_action("win.add-local-station", None, move |win, _, _| {
                let is_visible = win
                    .visible_dialog()
                    .map(|d| d.downcast::<SwAddStationDialog>().is_ok())
                    .unwrap_or(false);

                if !is_visible {
                    SwAddStationDialog::new().present(Some(win));
                }
            });
            klass.install_action("win.add-public-station", None, move |win, _, _| {
                win.show_uri("https://www.radio-browser.info/add");
            });
            klass.install_action("win.enable-gadget-player", None, move |win, _, _| {
                win.enable_gadget_player(true);
            });
            klass.install_action("win.disable-gadget-player", None, move |win, _, _| {
                win.enable_gadget_player(false);
            });
            klass.install_action("win.show-preferences", None, move |win, _, _| {
                let is_visible = win
                    .visible_dialog()
                    .map(|d| d.downcast::<SwPreferencesDialog>().is_ok())
                    .unwrap_or(false);

                if !is_visible {
                    SwPreferencesDialog::new().present(Some(win));
                }
            });
            klass.install_action("win.about", None, move |win, _, _| {
                let is_visible = win
                    .visible_dialog()
                    .map(|d| d.downcast::<adw::AboutDialog>().is_ok())
                    .unwrap_or(false);

                if !is_visible {
                    about_dialog::show(win);
                }
            });
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwApplicationWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            // Add devel style class for development or beta builds
            if config::PROFILE == "development" || config::PROFILE == "beta" {
                obj.add_css_class("devel");
            }

            // Restore window geometry
            let width = settings_manager::integer(Key::WindowWidth);
            let height = settings_manager::integer(Key::WindowHeight);
            obj.set_default_size(width, height);

            // Monitor window size changes for auto gadget mode
            let window_weak = obj.downgrade();
            obj.connect_default_height_notify(move |_window| {
                if let Some(window) = window_weak.upgrade() {
                    let height = window.default_height();
                    let gadget_visible = window.imp().player_gadget.is_visible();
                    
                    // Auto-switch to gadget mode if height is less than threshold
                    if height < 150 && !gadget_visible {
                        window.enable_gadget_player(true);
                    } else if height >= 150 && gadget_visible {
                        window.enable_gadget_player(false);
                    }
                }
            });
        }
    }

    impl WidgetImpl for SwApplicationWindow {}

    impl WindowImpl for SwApplicationWindow {
        fn close_request(&self) -> glib::Propagation {
            debug!("Saving window geometry.");
            let width = self.obj().default_size().0;
            let height = self.obj().default_size().1;

            settings_manager::set_integer(Key::WindowWidth, width);
            settings_manager::set_integer(Key::WindowHeight, height);

            let app = SwApplication::default();
            let player = app.player();

            if app.background_playback()
                && player.state() == SwPlaybackState::Playing
                && self.obj().is_visible()
            {
                let future = clone!(
                    #[weak(rename_to = imp)]
                    self,
                    async move {
                        imp.verify_background_portal_permissions().await;
                    }
                );
                glib::spawn_future_local(future);

                // We can't close the window immediately here, since we have to check first
                // whether we have background permissions. We just hide it, so we can show
                // it again if necessary.
                debug!("Hide window");
                self.obj().set_visible(false);

                glib::Propagation::Stop
            } else {
                debug!("Close window");
                glib::Propagation::Proceed
            }
        }
    }

    impl ApplicationWindowImpl for SwApplicationWindow {}

    impl AdwApplicationWindowImpl for SwApplicationWindow {}

    impl SwApplicationWindow {
        async fn verify_background_portal_permissions(&self) {
            // Verify whether app has permissions for background playback
            let has_permissions = utils::background_portal_permissions().await;
            let mut close_window = has_permissions;

            if !has_permissions {
                debug!("No background portal permissions, show window again.");
                self.obj().set_visible(true);

                let dialog = adw::AlertDialog::new(
                    Some(&i18n("No Permission for Background Playback")),
                    Some(&i18n(
                        "“Run in Background” must be allowed for this app in system settings.",
                    )),
                );

                dialog.add_response("try-anyway", &i18n("Try Anyway"));
                dialog.add_response("disable", &i18n("Disable Background Playback"));
                dialog.set_close_response("try-anyway");

                let res = dialog.choose_future(&*self.obj()).await;
                if res == "disable" {
                    SwApplication::default().set_background_playback(false);
                } else {
                    self.obj().set_visible(false);
                }
                close_window = true;
            }

            if close_window {
                self.obj().close();
            }
        }
    }
}

glib::wrapper! {
    pub struct SwApplicationWindow(
        ObjectSubclass<imp::SwApplicationWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl SwApplicationWindow {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn show_notification(&self, text: &str) {
        self.imp().toast_overlay.add_toast(adw::Toast::new(text));
    }

    pub fn enable_gadget_player(&self, enable: bool) {
        if enable {
            // Save current window size before entering gadget mode
            let (width, height) = self.default_size();
            settings_manager::set_integer(Key::WindowPreviousWidth, width);
            settings_manager::set_integer(Key::WindowPreviousHeight, height);

            self.imp().player_gadget.set_visible(true);
            self.imp().player_toolbar.set_visible(false);
        } else {
            // Restore initial window size from window manager settings
            let width = settings_manager::integer(Key::WindowWidth);
            let height = settings_manager::integer(Key::WindowHeight);
            self.set_default_size(width, height);

            self.imp().player_gadget.set_visible(false);
            self.imp().player_toolbar.set_visible(true);
        }
    }

    pub fn show_uri(&self, uri: &str) {
        let window = self.clone();
        let window_clone = window.clone();
        gtk::UriLauncher::new(uri).launch(Some(&window), None::<&gio::Cancellable>, move |res| {
            if let Err(err) = res {
                window_clone.show_notification(&err.to_string());
            }
        });
    }

    pub fn library_page(&self) -> SwLibraryPage {
        self.imp().library_page.get()
    }
}

impl Default for SwApplicationWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastWindow for SwApplicationWindow {
    fn toast_overlay(&self) -> adw::ToastOverlay {
        self.imp().toast_overlay.clone()
    }
}
