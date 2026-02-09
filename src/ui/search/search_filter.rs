// Shortwave - search_filter.rs
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

use std::sync::LazyLock;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::subclass;
use gtk::glib::subclass::Signal;
use gtk::{CompositeTemplate, glib};

use super::*;
use crate::api::StationRequest;

mod imp {
    use super::*;

    #[derive(Default, Debug, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/search_filter.ui")]
    pub struct SwSearchFilter {
        #[template_child]
        pub wrapbox: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub search_entry: TemplateChild<gtk::SearchEntry>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwSearchFilter {
        const NAME: &'static str = "SwSearchFilter";
        type ParentType = adw::Bin;
        type Type = super::SwSearchFilter;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwSearchFilter {
        fn signals() -> &'static [Signal] {
            static SIGNALS: LazyLock<Vec<Signal>> =
                LazyLock::new(|| vec![Signal::builder("filter-changed").build()]);

            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for SwSearchFilter {}

    impl BinImpl for SwSearchFilter {}

    #[gtk::template_callbacks]
    impl SwSearchFilter {
        #[template_callback]
        async fn add_filter(&self) {
            let item = SwSearchFilterItem::new("foo");
            self.wrapbox.append(&item);
        }

        #[template_callback]
        async fn search_changed(&self) {
            self.obj().emit_by_name::<()>("filter-changed", &[]);
        }

        #[template_callback]
        async fn stop_search(&self) {
            self.obj().activate_action("navigation.pop", None).unwrap();
        }
    }
}

glib::wrapper! {
    pub struct SwSearchFilter(ObjectSubclass<imp::SwSearchFilter>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SwSearchFilter {
    pub fn station_request(&self) -> StationRequest {
        let text = self.imp().search_entry.text().trim().to_string();
        let text = if text.is_empty() { None } else { Some(text) };

        StationRequest::search_for_name(text, 1000)
    }

    pub fn has_filter(&self) -> bool {
        !self.imp().search_entry.text().trim().is_empty()
    }

    pub fn grab_focus(&self) {
        self.imp().search_entry.grab_focus();
    }
}
