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

use crate::i18n::{i18n, ni18n_f};
use crate::settings::{settings_manager, Key};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/preferences_dialog.ui")]
    pub struct SwPreferencesDialog {
        // Playback
        #[template_child]
        notifications_switch: TemplateChild<gtk::Switch>,

        // Recording
        #[template_child]
        recording_track_directory_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        recording_maximum_duration_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        recording_minimum_duration_row: TemplateChild<adw::SpinRow>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwPreferencesDialog {
        const NAME: &'static str = "SwSettingsDialog";
        type ParentType = adw::PreferencesDialog;
        type Type = super::SwPreferencesDialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
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
                &*self.notifications_switch,
                "active",
            );

            // Recording
            let recording_mode_action = settings_manager::create_action(Key::RecordingMode);
            let group = gio::SimpleActionGroup::new();
            group.add_action(&recording_mode_action);
            self.obj().insert_action_group("player", Some(&group));

            settings_manager::bind_property(
                Key::RecordingTrackDirectory,
                &*self.recording_track_directory_row,
                "subtitle",
            );

            self.recording_track_directory_row.connect_activated(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_| {
                    imp.select_recording_save_directory();
                }
            ));

            settings_manager::bind_property(
                Key::RecordingMaximumDuration,
                &*self.recording_maximum_duration_row,
                "value",
            );

            self.recording_maximum_duration_row.connect_input(|row| {
                let mut text = row.text().to_string();
                text.retain(|c| c.is_numeric());

                if let Ok(value) = text.parse::<f64>() {
                    Some(Ok(value * 60.0))
                } else {
                    None
                }
            });

            settings_manager::bind_property(
                Key::RecordingMinimumDuration,
                &*self.recording_minimum_duration_row,
                "value",
            );
        }
    }

    impl WidgetImpl for SwPreferencesDialog {}

    impl AdwDialogImpl for SwPreferencesDialog {}

    impl PreferencesDialogImpl for SwPreferencesDialog {}

    #[gtk::template_callbacks]
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
                            Key::RecordingTrackDirectory,
                            folder.parse_name().to_string(),
                        );
                    }
                    Err(err) => {
                        warn!("Selected directory could not be accessed {:?}", err);
                    }
                },
            );
        }

        #[template_callback]
        fn on_maximum_duration_output(row: &adw::SpinRow) -> bool {
            let value = (row.value() / 60.0) as u32;
            let text = ni18n_f("{} min", "{} min", value, &[&value.to_string()]);
            row.set_text(&text);
            row.set_width_chars(text.len() as i32);
            true
        }

        #[template_callback]
        fn on_minimum_duration_output(row: &adw::SpinRow) -> bool {
            let value = row.value() as u32;
            let text = ni18n_f("{} sec", "{} sec", value, &[&value.to_string()]);
            row.set_text(&text);
            row.set_width_chars(text.len() as i32);
            true
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
