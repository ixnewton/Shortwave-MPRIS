// Shortwave - station_dialog.rs
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
use glib::{clone, subclass, Properties};
use gtk::{gdk, gio, glib, CompositeTemplate};
use url::Url;
use uuid::Uuid;

use crate::api::{StationMetadata, SwStation};
use crate::app::SwApplication;
use crate::i18n::i18n;
use crate::ui::SwStationCover;

mod imp {
    use super::*;

    #[derive(Debug, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/add_station_dialog.ui")]
    #[properties(wrapper_type = super::SwAddStationDialog)]
    pub struct SwAddStationDialog {
        #[template_child]
        add_button: TemplateChild<gtk::Button>,
        #[template_child]
        station_cover: TemplateChild<SwStationCover>,
        #[template_child]
        remove_cover_button: TemplateChild<gtk::Button>,
        #[template_child]
        name_row: TemplateChild<adw::EntryRow>,
        #[template_child]
        url_row: TemplateChild<adw::EntryRow>,

        #[property(get)]
        station: SwStation,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwAddStationDialog {
        const NAME: &'static str = "SwAddStationDialog";
        type ParentType = adw::Dialog;
        type Type = super::SwAddStationDialog;

        fn new() -> Self {
            let uuid = Uuid::new_v4().to_string();
            let metadata = StationMetadata::default();
            let station = SwStation::new(&uuid, true, metadata, None);

            Self {
                add_button: TemplateChild::default(),
                station_cover: TemplateChild::default(),
                remove_cover_button: TemplateChild::default(),
                name_row: TemplateChild::default(),
                url_row: TemplateChild::default(),
                station,
            }
        }

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwAddStationDialog {}

    impl WidgetImpl for SwAddStationDialog {}

    impl AdwDialogImpl for SwAddStationDialog {}

    #[gtk::template_callbacks]
    impl SwAddStationDialog {
        #[template_callback]
        fn select_cover_file(&self) {
            let file_chooser = gtk::FileDialog::builder()
                .title(i18n("Select Station Cover"))
                .build();

            let parent = self
                .obj()
                .root()
                .unwrap()
                .downcast::<gtk::Window>()
                .unwrap();

            file_chooser.open(
                Some(&parent),
                gio::Cancellable::NONE,
                clone!(
                    #[weak(rename_to = imp)]
                    self,
                    move |res| {
                        match res {
                            Ok(file) => match gdk::Texture::from_file(&file) {
                                Ok(texture) => {
                                    imp.obj().station().set_custom_cover(Some(texture));
                                    imp.remove_cover_button.set_visible(true);
                                }
                                Err(err) => {
                                    error!("Unable to open cover file: {}", err.to_string());
                                }
                            },
                            Err(err) => error!("Could not get file {err}"),
                        }
                    }
                ),
            );
        }

        #[template_callback]
        fn remove_cover(&self) {
            self.obj().station().set_custom_cover(gdk::Texture::NONE);
            self.remove_cover_button.set_visible(false);
        }

        #[template_callback]
        fn add_station(&self) {
            SwApplication::default()
                .library()
                .add_station(self.obj().station());

            self.obj().close();
        }

        #[template_callback]
        fn update_metadata(&self) {
            let name = self.name_row.text().to_string();
            let has_name = !name.is_empty();
            let url = Url::parse(&self.url_row.text()).ok();

            match url {
                Some(_) => {
                    self.url_row.remove_css_class("error");
                    self.add_button.set_sensitive(has_name);
                }
                None => {
                    self.url_row.add_css_class("error");
                    self.add_button.set_sensitive(false);
                }
            }

            let metadata = StationMetadata {
                name,
                url,
                ..Default::default()
            };
            self.obj().station().set_metadata(metadata);
        }
    }
}

glib::wrapper! {
    pub struct SwAddStationDialog(ObjectSubclass<imp::SwAddStationDialog>)
        @extends gtk::Widget, adw::Dialog,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SwAddStationDialog {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for SwAddStationDialog {
    fn default() -> Self {
        Self::new()
    }
}
