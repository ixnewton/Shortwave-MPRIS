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
use glib::subclass;
use gtk::{gio, glib, CompositeTemplate};

use crate::app::SwApplication;
use crate::config;
use crate::settings::{settings_manager, Key};
use crate::ui::pages::*;
use crate::ui::player::{SwPlayerGadget, SwPlayerToolbar, SwPlayerView};
use crate::ui::{
    about_dialog, DisplayError, SwAddStationDialog, SwDeviceDialog, SwPreferencesDialog,
    SwStationDialog, ToastWindow,
};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/window.ui")]
    pub struct SwApplicationWindow {
        #[template_child]
        library_page: TemplateChild<SwLibraryPage>,
        #[template_child]
        search_page: TemplateChild<SwSearchPage>,

        #[template_child]
        player_gadget: TemplateChild<SwPlayerGadget>,
        #[template_child]
        player_toolbar: TemplateChild<SwPlayerToolbar>,
        #[template_child]
        player_view: TemplateChild<SwPlayerView>,
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
                SwDeviceDialog::new().present(Some(win));
            });
            klass.install_action("player.show-station-details", None, move |win, _, _| {
                if let Some(station) = SwApplication::default().player().station() {
                    SwStationDialog::new(&station).present(Some(win));
                }
            });

            // win
            klass.install_action("win.open-radio-browser-info", None, move |win, _, _| {
                win.show_uri("https://www.radio-browser.info/");
            });
            klass.install_action("win.add-local-station", None, move |win, _, _| {
                SwAddStationDialog::new().present(Some(win));
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
                SwPreferencesDialog::new().present(Some(win));
            });
            klass.install_action("win.about", None, move |win, _, _| {
                about_dialog::show(win);
            });
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
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

            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for SwApplicationWindow {}

    impl AdwApplicationWindowImpl for SwApplicationWindow {}

    impl SwApplicationWindow {}
}

glib::wrapper! {
    pub struct SwApplicationWindow(
        ObjectSubclass<imp::SwApplicationWindow>)
        @extends gtk::Widget, gtk::Window, gtk::ApplicationWindow, adw::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup;
}

impl SwApplicationWindow {
    pub fn new() -> Self {
        glib::Object::new::<Self>()
    }

    pub fn show_notification(&self, text: &str) {
        let toast = adw::Toast::new(text);
        self.imp().toast_overlay.add_toast(toast);
    }

    pub fn enable_gadget_player(&self, enable: bool) {
        debug!("Enable gadget player: {:?}", enable);

        if self.is_maximized() && enable {
            self.unmaximize();
            return;
        }

        let mut previous_width = settings_manager::integer(Key::WindowPreviousWidth) as f64;
        let mut previous_height = settings_manager::integer(Key::WindowPreviousHeight) as f64;

        // Save current window size as previous size, so you can restore it
        // if you switch between gadget player / normal window mode.
        let current_width = self.default_size().0;
        let current_height = self.default_size().1;

        settings_manager::set_integer(Key::WindowPreviousWidth, current_width);
        settings_manager::set_integer(Key::WindowPreviousHeight, current_height);

        if enable && previous_height > 175.0 {
            previous_width = 450.0;
            previous_height = 105.0;
        } else if !enable && previous_height < 175.0 {
            previous_width = 950.0;
            previous_height = 650.0;
        }

        self.set_visible(false);
        self.set_default_height(previous_height as i32);
        self.set_default_width(previous_width as i32);
        self.set_visible(true);
    }

    pub fn show_uri(&self, uri: &str) {
        let launcher = gtk::UriLauncher::new(uri);
        launcher.launch(Some(self), gio::Cancellable::NONE, |res| {
            res.handle_error("Unable to launch URI");
        });
    }
}

impl Default for SwApplicationWindow {
    fn default() -> Self {
        SwApplication::default()
            .active_window()
            .unwrap()
            .downcast()
            .unwrap()
    }
}

impl ToastWindow for SwApplicationWindow {
    fn toast_overlay(&self) -> adw::ToastOverlay {
        self.imp().toast_overlay.clone()
    }
}
