// Shortwave - search_filter_item.rs
// Copyright (C) 2025  Felix Häcker <haeckerfelix@gnome.org>
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
use glib::Properties;
use glib::subclass;
use gtk::glib;

mod imp {
    use super::*;

    #[derive(Default, Debug, Properties)]
    #[properties(wrapper_type = super::SwSearchFilterItem)]
    pub struct SwSearchFilterItem {
        #[property(get, set)]
        value: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwSearchFilterItem {
        const NAME: &'static str = "SwSearchFilterItem";
        type ParentType = gtk::Button;
        type Type = super::SwSearchFilterItem;

        fn class_init(_klass: &mut Self::Class) {}

        fn instance_init(_obj: &subclass::InitializingObject<Self>) {}
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwSearchFilterItem {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().add_css_class("circular");

            let box_ = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            box_.set_margin_start(12);
            box_.set_margin_end(12);

            let image = gtk::Image::from_icon_name("remove-filter-symbolic");

            let label = gtk::Label::new(None);
            self.obj().bind_property("value", &label, "label").build();

            box_.append(&label);
            box_.append(&image);

            self.obj().set_child(Some(&box_));
        }
    }

    impl WidgetImpl for SwSearchFilterItem {}

    impl ButtonImpl for SwSearchFilterItem {}

    impl SwSearchFilterItem {}
}

glib::wrapper! {
    pub struct SwSearchFilterItem(ObjectSubclass<imp::SwSearchFilterItem>)
        @extends gtk::Widget, gtk::Button,
        @implements gtk::Actionable, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SwSearchFilterItem {
    pub fn new(value: &str) -> Self {
        glib::Object::builder().property("value", value).build()
    }
}
