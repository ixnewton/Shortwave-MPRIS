// Shortwave - station_row.rs
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

use futures_util::future::FutureExt;
use glib::clone;
use glib::Properties;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate};
use inflector::Inflector;

use crate::api::{FaviconDownloader, SwStation};
use crate::ui::SwFavicon;
use crate::SwApplication;

mod imp {
    use glib::subclass;

    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/station_row.ui")]
    #[properties(wrapper_type = super::SwStationRow)]
    pub struct SwStationRow {
        #[template_child]
        station_label: TemplateChild<gtk::Label>,
        #[template_child]
        subtitle_label: TemplateChild<gtk::Label>,
        #[template_child]
        station_favicon: TemplateChild<SwFavicon>,
        #[template_child]
        local_image: TemplateChild<gtk::Image>,
        #[template_child]
        orphaned_image: TemplateChild<gtk::Image>,
        #[template_child]
        play_button: TemplateChild<gtk::Button>,

        #[property(get, set, construct_only)]
        station: OnceCell<SwStation>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStationRow {
        const NAME: &'static str = "SwStationRow";
        type ParentType = gtk::FlowBoxChild;
        type Type = super::SwStationRow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwStationRow {
        fn constructed(&self) {
            self.parent_constructed();
            let station = self.obj().station();

            // Download & set station favicon
            if let Some(texture) = station.favicon() {
                self.station_favicon.set_paintable(Some(&texture.upcast()));
            } else if let Some(favicon) = station.metadata().favicon.as_ref() {
                let fut = FaviconDownloader::download(favicon.clone()).map(clone!(
                    #[weak(rename_to = imp)]
                    self,
                    move |paintable| {
                        if let Ok(paintable) = paintable {
                            imp.station_favicon.set_paintable(Some(&paintable))
                        }
                    }
                ));
                glib::spawn_future_local(fut);
            }

            station
                .bind_property("is-local", &*self.local_image, "visible")
                .sync_create()
                .build();

            station
                .bind_property("is-orphaned", &*self.orphaned_image, "visible")
                .sync_create()
                .build();

            station.connect_metadata_notify(clone!(
                #[weak(rename_to = this)]
                self,
                move |_| {
                    this.set_metadata();
                }
            ));
            self.set_metadata();

            self.play_button.connect_clicked(clone!(
                #[strong(rename_to = station)]
                self.station,
                move |_| {
                    SwApplication::default()
                        .player()
                        .set_station(station.get().unwrap().clone());
                }
            ));
        }
    }

    impl WidgetImpl for SwStationRow {}

    impl FlowBoxChildImpl for SwStationRow {}

    impl SwStationRow {
        fn set_metadata(&self) {
            let station = self.obj().station();
            let metadata = station.metadata();

            self.station_label.set_text(&metadata.name);
            let mut subtitle = metadata.country.to_title_case();

            if subtitle.is_empty() {
                subtitle = metadata.tags;
            } else if !metadata.tags.is_empty() {
                subtitle = format!("{} · {}", subtitle, metadata.formatted_tags());
            }

            self.subtitle_label.set_text(&subtitle);
            self.subtitle_label.set_visible(!subtitle.is_empty());
        }
    }
}

glib::wrapper! {
    pub struct SwStationRow(ObjectSubclass<imp::SwStationRow>)
        @extends gtk::Widget, gtk::FlowBoxChild;
}

impl SwStationRow {
    pub fn new(station: &SwStation) -> Self {
        glib::Object::builder().property("station", station).build()
    }
}
