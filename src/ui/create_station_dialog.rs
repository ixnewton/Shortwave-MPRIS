// Shortwave - station_dialog.rs
// Copyright (C) 2021-2023  Felix Häcker <haeckerfelix@gnome.org>
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

use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass};
use gtk::{gdk, gio, glib, CompositeTemplate};
use url::Url;
use uuid::Uuid;

use crate::api::{StationMetadata, SwStation};
use crate::app::SwApplication;
use crate::i18n::i18n;
use crate::ui::{SwApplicationWindow, SwFavicon};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/create_station_dialog.ui")]
    pub struct SwCreateStationDialog {
        #[template_child]
        pub create_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub station_favicon: TemplateChild<SwFavicon>,
        #[template_child]
        pub favicon_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub name_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        pub url_row: TemplateChild<adw::EntryRow>,

        pub favicon: RefCell<Option<gtk::gdk::Texture>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwCreateStationDialog {
        const NAME: &'static str = "SwCreateStationDialog";
        type ParentType = adw::Dialog;
        type Type = super::SwCreateStationDialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::Type::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwCreateStationDialog {}

    impl WidgetImpl for SwCreateStationDialog {}

    impl AdwDialogImpl for SwCreateStationDialog {}
}

glib::wrapper! {
    pub struct SwCreateStationDialog(ObjectSubclass<imp::SwCreateStationDialog>)
        @extends gtk::Widget, adw::Dialog;
}

#[gtk::template_callbacks]
impl SwCreateStationDialog {
    pub fn new() -> Self {
        let dialog: Self = glib::Object::new();

        dialog.setup_signals();
        dialog
    }

    fn show_filechooser(&self) {
        let file_chooser = gtk::FileDialog::builder()
            .title(i18n("Select Station Cover"))
            .build();

        file_chooser.open(
            Some(&SwApplicationWindow::default()),
            gio::Cancellable::NONE,
            clone!(
                #[weak(rename_to = imp)]
                self,
                move |res| {
                    match res {
                        Ok(file) => imp.set_favicon(&file),
                        Err(err) => error!("Could not get file {err}"),
                    }
                }
            ),
        );
    }

    fn setup_signals(&self) {
        let imp = self.imp();

        imp.favicon_button.connect_clicked(clone!(
            #[weak(rename_to = imp)]
            self,
            move |_| {
                imp.show_filechooser();
            }
        ));
    }

    #[template_callback]
    fn create_station(&self) {
        let imp = self.imp();
        let uuid = Uuid::new_v4().to_string();
        let name = imp.name_row.text().to_string();
        let url = Url::parse(&imp.url_row.text()).unwrap();
        let favicon = imp.favicon.borrow().clone();

        let station = SwStation::new(
            &uuid,
            true,
            StationMetadata::new(name, url),
            favicon.and_upcast(),
        );
        SwApplication::default().library().add_station(station);
        self.close();
    }

    #[template_callback]
    fn validate_input(&self) {
        let imp = self.imp();

        let has_name = !imp.name_row.text().is_empty();
        let url = imp.url_row.text().to_string();

        match Url::parse(&url) {
            Ok(_) => {
                imp.url_row.remove_css_class("error");
                imp.create_button.set_sensitive(has_name);
            }
            Err(_) => {
                imp.url_row.add_css_class("error");
                imp.create_button.set_sensitive(false);
            }
        }
    }

    fn set_favicon(&self, file: &gio::File) {
        if let Ok(texture) = gdk::Texture::from_file(file) {
            self.imp()
                .station_favicon
                .set_paintable(Some(&texture.clone().upcast()));
            self.imp().favicon.replace(Some(texture));
        }
    }
}

impl Default for SwCreateStationDialog {
    fn default() -> Self {
        Self::new()
    }
}
