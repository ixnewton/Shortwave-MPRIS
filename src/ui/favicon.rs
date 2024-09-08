// Shortwave - favicon.rs
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

use std::cell::Cell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{subclass, Enum, Properties};
use gtk::{gdk, glib, CompositeTemplate};

#[derive(Display, Copy, Debug, Clone, EnumString, Eq, PartialEq, Enum)]
#[repr(u32)]
#[enum_type(name = "SwFaviconSize")]
#[derive(Default)]
pub enum SwFaviconSize {
    #[default]
    Mini,
    Small,
    Big,
}

mod imp {
    use std::marker::PhantomData;

    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/favicon.ui")]
    #[properties(wrapper_type = super::SwFavicon)]
    pub struct SwFavicon {
        #[template_child]
        image: TemplateChild<gtk::Image>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        placeholder: TemplateChild<gtk::Image>,

        #[property(get=Self::paintable, set=Self::set_paintable, nullable)]
        paintable: PhantomData<Option<gdk::Paintable>>,
        #[property(get, set, construct_only, builder(SwFaviconSize::default()))]
        size: Cell<SwFaviconSize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwFavicon {
        const NAME: &'static str = "SwFavicon";
        type ParentType = adw::Bin;
        type Type = super::SwFavicon;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("favicon");
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwFavicon {
        fn constructed(&self) {
            self.parent_constructed();

            let size = match self.obj().size() {
                SwFaviconSize::Mini => 48,
                SwFaviconSize::Small => 64,
                SwFaviconSize::Big => {
                    self.image.add_css_class("card");
                    self.placeholder.add_css_class("card");
                    192
                }
            };

            self.image.set_size_request(size, size);
            self.placeholder.set_pixel_size(size.div_euclid(2));
        }
    }

    impl WidgetImpl for SwFavicon {}

    impl BinImpl for SwFavicon {}

    impl SwFavicon {
        pub fn paintable(&self) -> Option<gdk::Paintable> {
            self.image.paintable()
        }

        pub fn set_paintable(&self, paintable: Option<&gdk::Paintable>) {
            self.image.set_paintable(paintable);

            let n = if paintable.is_some() {
                "image"
            } else {
                "placeholder"
            };
            self.stack.set_visible_child_name(n);
        }
    }
}

glib::wrapper! {
    pub struct SwFavicon(ObjectSubclass<imp::SwFavicon>)
        @extends gtk::Widget, adw::Bin;
}

#[gtk::template_callbacks]
impl SwFavicon {
    pub fn new(size: SwFaviconSize) -> Self {
        glib::Object::builder().property("size", size).build()
    }

    pub fn reset(&self) {
        self.imp().stack.set_visible_child_name("placeholder");
    }
}
