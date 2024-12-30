// Shortwave - preferences_dialog.rs
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

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass};
use gtk::{gio, glib, CompositeTemplate};

use crate::i18n::i18n;
use crate::settings::{settings_manager, Key};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/preferences_dialog.ui")]
    pub struct SwPreferencesDialog {
        // Playback
        #[template_child]
        show_notifications_button: TemplateChild<gtk::Switch>,

        // Recording
        #[template_child]
        track_save_path_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        track_duration_threshold_row: TemplateChild<adw::SpinRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwPreferencesDialog {
        const NAME: &'static str = "SwSettingsDialog";
        type ParentType = adw::PreferencesDialog;
        type Type = super::SwPreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwPreferencesDialog {
        fn constructed(&self) {
            // Playback
            settings_manager::bind_property(
                Key::Notifications,
                &*self.show_notifications_button,
                "active",
            );

            // Recording
            let recording_mode_action = settings_manager::create_action(Key::RecordingMode);
            let group = gio::SimpleActionGroup::new();
            group.add_action(&recording_mode_action);
            self.obj().insert_action_group("player", Some(&group));

            settings_manager::bind_property(
                Key::RecorderSongSavePath,
                &*self.track_save_path_row,
                "subtitle",
            );

            self.track_save_path_row.connect_activated(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_| {
                    imp.select_recording_save_directory();
                }
            ));

            settings_manager::bind_property(
                Key::RecorderSongDurationThreshold,
                &*self.track_duration_threshold_row,
                "value",
            );
        }
    }

    impl WidgetImpl for SwPreferencesDialog {}

    impl AdwDialogImpl for SwPreferencesDialog {}

    impl PreferencesDialogImpl for SwPreferencesDialog {}

    impl SwPreferencesDialog {
        pub fn select_recording_save_directory(&self) {
            let parent = self
                .obj()
                .root()
                .unwrap()
                .downcast::<gtk::Window>()
                .unwrap();

            let dialog = gtk::FileDialog::new();
            dialog.set_title(&i18n("Select Save Directory"));
            dialog.set_accept_label(Some(&i18n("_Select")));

            dialog.select_folder(
                Some(&parent),
                gio::Cancellable::NONE,
                move |result| match result {
                    Ok(folder) => {
                        debug!("Selected save directory: {:?}", folder.path());
                        settings_manager::set_string(
                            Key::RecorderSongSavePath,
                            folder.parse_name().to_string(),
                        );
                    }
                    Err(err) => {
                        warn!("Selected directory could not be accessed {:?}", err);
                    }
                },
            );
        }
    }
}

glib::wrapper! {
    pub struct SwPreferencesDialog(ObjectSubclass<imp::SwPreferencesDialog>)
        @extends gtk::Widget, adw::Dialog, adw::PreferencesDialog;
}

impl Default for SwPreferencesDialog {
    fn default() -> Self {
        glib::Object::new()
    }
}
