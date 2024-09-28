// Shortwave - window.rs
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

use std::cell::OnceCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass};
use gtk::{gio, glib, CompositeTemplate};

use crate::app::SwApplication;
use crate::config;
use crate::settings::{settings_manager, Key};
use crate::ui::pages::*;
use crate::ui::player::{SwPlayerGadget, SwPlayerToolbar, SwPlayerView};
use crate::ui::{DisplayError, SwCreateStationDialog, SwDeviceDialog, SwStationDialog};

use super::ToastWindow;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/window.ui")]
    pub struct SwApplicationWindow {
        #[template_child]
        pub library_page: TemplateChild<SwLibraryPage>,
        #[template_child]
        pub search_page: TemplateChild<SwSearchPage>,

        #[template_child]
        pub player_gadget: TemplateChild<SwPlayerGadget>,
        #[template_child]
        pub player_toolbar: TemplateChild<SwPlayerToolbar>,
        #[template_child]
        pub player_view: TemplateChild<SwPlayerView>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,

        pub window_animation_x: OnceCell<adw::TimedAnimation>,
        pub window_animation_y: OnceCell<adw::TimedAnimation>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwApplicationWindow {
        const NAME: &'static str = "SwApplicationWindow";
        type ParentType = adw::ApplicationWindow;
        type Type = super::SwApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwApplicationWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            let player_actions = gio::SimpleActionGroup::new();

            // player.start-playback
            let a = gio::SimpleAction::new("start-playback", None);
            a.connect_activate(move |_, _| {
                glib::spawn_future_local(async move {
                    SwApplication::default().player().start_playback().await;
                });
            });
            player_actions.add_action(&a);

            // player.stop-playback
            let a = gio::SimpleAction::new("stop-playback", None);
            a.connect_activate(move |_, _| {
                glib::spawn_future_local(async move {
                    SwApplication::default().player().stop_playback().await;
                });
            });
            player_actions.add_action(&a);

            // player.toggle-playback
            let a = gio::SimpleAction::new("toggle-playback", None);
            a.connect_activate(move |_, _| {
                glib::spawn_future_local(async move {
                    SwApplication::default().player().toggle_playback().await;
                });
            });
            player_actions.add_action(&a);

            // player.show-device-connect
            let a = gio::SimpleAction::new("show-device-connect", None);
            a.connect_activate(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_, _| {
                    SwDeviceDialog::new().present(Some(&*imp.obj()));
                }
            ));
            player_actions.add_action(&a);

            // player.show-station-details
            let a = gio::SimpleAction::new("show-station-details", None);
            a.connect_activate(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_, _| {
                    if let Some(station) = SwApplication::default().player().station() {
                        SwStationDialog::new(&station).present(Some(&*imp.obj()));
                    }
                }
            ));
            player_actions.add_action(&a);

            obj.insert_action_group("player", Some(&player_actions));

            self.obj().setup_widgets();
            self.obj().setup_gactions();
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

    pub fn setup_widgets(&self) {
        let imp = self.imp();

        // Animations for smooth gadget player transitions
        let x_callback = adw::CallbackAnimationTarget::new(clone!(
            #[weak(rename_to = this)]
            self,
            move |val| {
                this.set_default_width(val as i32);
            }
        ));
        let x_animation = adw::TimedAnimation::new(self, 0.0, 0.0, 500, x_callback);
        x_animation.set_easing(adw::Easing::EaseOutCubic);
        imp.window_animation_x.set(x_animation).unwrap();

        let y_callback = adw::CallbackAnimationTarget::new(clone!(
            #[weak(rename_to = this)]
            self,
            move |val| {
                this.set_default_height(val as i32);
            }
        ));
        let y_animation = adw::TimedAnimation::new(self, 0.0, 0.0, 500, y_callback);
        y_animation.set_easing(adw::Easing::EaseOutCubic);
        imp.window_animation_y.set(y_animation).unwrap();

        // Add devel style class for development or beta builds
        if config::PROFILE == "development" || config::PROFILE == "beta" {
            self.add_css_class("devel");
        }

        // Restore window geometry
        let width = settings_manager::integer(Key::WindowWidth);
        let height = settings_manager::integer(Key::WindowHeight);
        self.set_default_size(width, height);
    }

    fn setup_gactions(&self) {
        let app = SwApplication::default();

        self.add_action_entries([
            // win.open-radio-browser-info
            gio::ActionEntry::builder("open-radio-browser-info")
                .activate(|window: &Self, _, _| {
                    window.show_uri("https://www.radio-browser.info/");
                })
                .build(),
            // win.create-new-station
            gio::ActionEntry::builder("create-new-station")
                .activate(move |window: &Self, _, _| {
                    let dialog = SwCreateStationDialog::new();
                    dialog.present(Some(window));
                })
                .build(),
            // win.disable-gadget-player
            gio::ActionEntry::builder("disable-gadget-player")
                .activate(move |window: &Self, _, _| {
                    window.enable_gadget_player(false);
                })
                .build(),
            // win.enable-gadget-player
            gio::ActionEntry::builder("enable-gadget-player")
                .activate(move |window: &Self, _, _| {
                    window.enable_gadget_player(true);
                })
                .build(),
        ]);
        app.set_accels_for_action("player.toggle-playback", &["<primary>space"]);
    }

    pub fn show_notification(&self, text: &str) {
        let toast = adw::Toast::new(text);
        self.imp().toast_overlay.add_toast(toast);
    }

    pub fn enable_gadget_player(&self, enable: bool) {
        debug!("Enable gadget player: {:?}", enable);

        if self.is_maximized() && enable {
            self.unmaximize();
        }

        let mut previous_width = settings_manager::integer(Key::WindowPreviousWidth) as f64;
        let mut previous_height = settings_manager::integer(Key::WindowPreviousHeight) as f64;

        // Save current window size as previous size, so you can restore it
        // if you switch between gadget player / normal window mode.
        let current_width = self.default_size().0;
        let current_height = self.default_size().1;
        settings_manager::set_integer(Key::WindowPreviousWidth, current_width);
        settings_manager::set_integer(Key::WindowPreviousHeight, current_height);

        let x_animation = self.imp().window_animation_x.get().unwrap();
        let y_animation = self.imp().window_animation_y.get().unwrap();

        x_animation.reset();
        x_animation.set_value_from(self.width() as f64);
        y_animation.reset();
        y_animation.set_value_from(self.height() as f64);

        if enable {
            if previous_height > 175.0 {
                previous_width = 450.0;
                previous_height = 105.0;
            }

            x_animation.set_value_to(previous_width);
            y_animation.set_value_to(previous_height);
        } else {
            if previous_height < 175.0 {
                previous_width = 950.0;
                previous_height = 650.0;
            }

            x_animation.set_value_to(previous_width);
            y_animation.set_value_to(previous_height);
        }

        x_animation.play();
        y_animation.play();
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
