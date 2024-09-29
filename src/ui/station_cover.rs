// Shortwave - station_cover.rs
// Copyright (C) 2024  Felix Häcker <haeckerfelix@gnome.org>
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

use std::cell::Cell;
use std::cell::RefCell;
use std::hash::{DefaultHasher, Hash, Hasher};

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass, Properties};
use gtk::{gio, glib, pango, CompositeTemplate};

use crate::api::SwStation;
use crate::app::SwApplication;
use crate::config;

mod imp {
    use super::*;
    static MAX_COVER_SIZE: i32 = 256;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/station_cover.ui")]
    #[properties(wrapper_type = super::SwStationCover)]
    pub struct SwStationCover {
        #[template_child]
        placeholder_image: TemplateChild<gtk::Image>,
        #[template_child]
        image: TemplateChild<gtk::Image>,
        #[template_child]
        stack: TemplateChild<gtk::Stack>,
        #[template_child]
        fallback_label: TemplateChild<gtk::Label>,

        #[property(get, set, construct_only)]
        size: Cell<i32>,
        #[property(get, set=Self::set_station)]
        station: RefCell<Option<SwStation>>,
        #[property(get)]
        is_loaded: Cell<bool>,

        loader_cancellable: RefCell<Option<gio::Cancellable>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStationCover {
        const NAME: &'static str = "SwStationCover";
        type ParentType = adw::Bin;
        type Type = super::SwStationCover;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("cover");
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwStationCover {
        fn constructed(&self) {
            self.parent_constructed();

            let size = self.obj().size();
            self.image.set_size_request(size, size);
            self.obj().set_size_request(size, size);

            self.placeholder_image.set_pixel_size(size.div_euclid(2));

            let icon = format!("{}-symbolic", config::APP_ID);
            self.placeholder_image.set_icon_name(Some(&icon));

            self.update_initials();
            self.update_font_size();
            self.update_color_class();
        }
    }

    impl WidgetImpl for SwStationCover {
        fn map(&self) {
            self.parent_map();
            glib::spawn_future_local(clone!(
                #[weak(rename_to = imp)]
                self,
                async move {
                    imp.load_cover().await;
                }
            ));
        }

        fn unmap(&self) {
            self.parent_unmap();
            self.cancel();
        }
    }

    impl BinImpl for SwStationCover {}

    impl SwStationCover {
        fn set_station(&self, station: Option<&SwStation>) {
            *self.station.borrow_mut() = station.cloned();

            // Reset previous cover
            self.image.set_paintable(gtk::gdk::Paintable::NONE);
            self.stack.set_visible_child_name("fallback");

            self.is_loaded.set(false);
            self.obj().notify_is_loaded();

            // Set fallback initials
            self.update_initials();
            self.update_font_size();
            self.update_color_class();

            // Load new cover, but only if it's mapped
            if self.obj().is_mapped() {
                glib::spawn_future_local(clone!(
                    #[weak(rename_to = imp)]
                    self,
                    async move {
                        imp.load_cover().await;
                    }
                ));
            }
        }

        fn update_initials(&self) {
            let title = if let Some(station) = self.obj().station() {
                station.title()
            } else {
                String::new()
            };

            let mut initials = String::new();
            let words: Vec<&str> = title.split(" ").collect();

            if let Some(char) = words.first().and_then(|w| Self::first_char(w)) {
                initials += &char.to_string();
            }

            if let Some(char) = words.get(1).and_then(|w| Self::first_char(w)) {
                initials += &char.to_string();
            }

            if initials.is_empty() {
                initials += "?";
            }

            self.fallback_label.set_label(&initials.to_uppercase());
        }

        fn first_char(word: &str) -> Option<char> {
            word.chars()
                .filter(|c| c.is_alphabetic())
                .collect::<Vec<char>>()
                .first()
                .cloned()
        }

        fn update_font_size(&self) {
            let attributes = pango::AttrList::new();
            self.fallback_label.set_attributes(Some(&attributes));

            let (width, height) = self.fallback_label.layout().pixel_size();

            let size = self.obj().size() as f32;
            let padding = f32::max(self.obj().size() as f32 * 0.5, 0.0);
            let max_size: f32 = size - padding;
            let new_font_size = height as f32 * (max_size / width as f32);

            attributes.insert(pango::AttrSize::new_size_absolute(
                (new_font_size.clamp(0.0, max_size) * pango::SCALE as f32) as i32,
            ));
            self.fallback_label.set_attributes(Some(&attributes));
        }

        fn update_color_class(&self) {
            for css_class in self.fallback_label.css_classes() {
                self.fallback_label.remove_css_class(&css_class);
            }

            if let Some(station) = self.obj().station() {
                let mut hasher = DefaultHasher::new();
                station.title().hash(&mut hasher);
                let hash = hasher.finish();

                let color_class = hash % 14;
                self.fallback_label
                    .add_css_class(&format!("color{color_class}"));
            }
        }

        async fn load_cover(&self) {
            if self.obj().is_loaded() || !self.obj().is_mapped() {
                return;
            }

            self.cancel();

            if let Some(station) = self.obj().station() {
                if let Some(favicon_url) = station.metadata().favicon {
                    let mut cover_loader = SwApplication::default().cover_loader();

                    let cancellable = gio::Cancellable::new();
                    *self.loader_cancellable.borrow_mut() = Some(cancellable.clone());

                    let size = MAX_COVER_SIZE * self.obj().scale_factor();
                    let res = cover_loader
                        .load_cover(&favicon_url, size, cancellable.clone())
                        .await;

                    match res {
                        Ok(texture) => {
                            self.image.set_paintable(Some(&texture));
                            self.stack.set_visible_child_name("image");

                            self.is_loaded.set(true);
                            self.obj().notify_is_loaded();
                        }
                        Err(e) => {
                            if e.root_cause().to_string() != "cancelled" {
                                warn!(
                                    "Unable to load cover for station {:?} ({:?}): {}",
                                    station.title(),
                                    station.metadata().favicon.map(|f| f.to_string()),
                                    e.root_cause().to_string()
                                )
                            }
                        }
                    }
                }
            } else {
                self.stack.set_visible_child_name("placeholder");
            }
        }

        fn cancel(&self) {
            if let Some(cancellable) = self.loader_cancellable.borrow_mut().take() {
                cancellable.cancel();
            }
        }
    }
}

glib::wrapper! {
    pub struct SwStationCover(ObjectSubclass<imp::SwStationCover>)
        @extends gtk::Widget, adw::Bin;
}
