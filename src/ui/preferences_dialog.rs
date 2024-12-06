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

use adw::subclass::prelude::*;
use glib::subclass;
use gtk::{glib, CompositeTemplate};

use crate::settings::{settings_manager, Key};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/preferences_dialog.ui")]
    pub struct SwPreferencesDialog {
        #[template_child]
        show_notifications_button: TemplateChild<gtk::Switch>,
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
            settings_manager::bind_property(
                Key::Notifications,
                &*self.show_notifications_button,
                "active",
            );
        }
    }

    impl WidgetImpl for SwPreferencesDialog {}

    impl AdwDialogImpl for SwPreferencesDialog {}

    impl PreferencesDialogImpl for SwPreferencesDialog {}
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
