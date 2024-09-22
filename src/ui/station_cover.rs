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

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass, Properties};
use gtk::{gio, glib, CompositeTemplate};

use crate::api::SwStation;
use crate::app::SwApplication;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/station_cover.ui")]
    #[properties(wrapper_type = super::SwStationCover)]
    pub struct SwStationCover {
        #[template_child]
        image: TemplateChild<gtk::Image>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        placeholder: TemplateChild<gtk::Image>,

        #[property(get, set, construct_only)]
        size: Cell<i32>,
        #[property(get, set)]
        station: RefCell<Option<SwStation>>,

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
            self.placeholder.set_pixel_size(size.div_euclid(2));
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
            self.cancel_request();
        }
    }

    impl BinImpl for SwStationCover {}

    impl SwStationCover {
        async fn load_cover(&self) {
            self.cancel_request();

            if let Some(station) = self.obj().station() {
                let mut cover_loader = SwApplication::default().cover_loader();
                let cancellable = gio::Cancellable::new();

                match cover_loader
                    .load_cover(&station, self.obj().size(), cancellable.clone())
                    .await
                {
                    Ok(Some(texture)) => {
                        self.image.set_paintable(Some(&texture));
                        self.stack.set_visible_child_name("image");
                    }
                    Err(err) => {
                        warn!(
                            "Unable to load station cover ({:?}) ({}): {}",
                            station.title(),
                            station.metadata().favicon.unwrap(),
                            err.root_cause().to_string()
                        )
                    }
                    _ => (), // Cancelled
                }
                *self.loader_cancellable.borrow_mut() = Some(cancellable);
            }
        }

        fn cancel_request(&self) {
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
