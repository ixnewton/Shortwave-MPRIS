// Shortwave - search_page.rs
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

use adw::subclass::prelude::*;
use glib::{clone, subclass, Properties};
use gtk::prelude::*;
use gtk::{glib, CompositeTemplate};

use crate::api::{SwStationSorting, SwStationSortingType};
use crate::app::SwApplication;
use crate::config;
use crate::database::SwLibraryStatus;
use crate::i18n::*;
use crate::settings::{settings_manager, Key};
use crate::ui::SwStationFlowBox;

mod imp {
    use super::*;

    #[derive(Default, Debug, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/library_page.ui")]
    #[properties(wrapper_type = super::SwLibraryPage)]
    pub struct SwLibraryPage {
        #[template_child]
        status_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        stack: TemplateChild<gtk::Stack>,
        #[template_child]
        flowbox: TemplateChild<SwStationFlowBox>,

        #[property(get, set, builder(SwStationSorting::default()))]
        sorting: Cell<SwStationSorting>,
        #[property(get, set, builder(SwStationSortingType::Ascending))]
        sorting_type: Cell<SwStationSortingType>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwLibraryPage {
        const NAME: &'static str = "SwLibraryPage";
        type ParentType = adw::NavigationPage;
        type Type = super::SwLibraryPage;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            klass.install_property_action("library.set-sorting", "sorting");
            klass.install_property_action("library.set-sorting-type", "sorting-type");
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwLibraryPage {
        fn constructed(&self) {
            self.parent_constructed();
            let library = SwApplication::default().library();

            settings_manager::bind_property(Key::LibrarySorting, &*self.obj(), "sorting");
            settings_manager::bind_property(Key::LibrarySortingType, &*self.obj(), "sorting-type");

            self.obj()
                .bind_property("sorting", &self.flowbox.sorter(), "sorting")
                .bidirectional()
                .build();

            self.obj()
                .bind_property("sorting-type", &self.flowbox.sorter(), "sorting-type")
                .bidirectional()
                .build();

            // Setup empty state page
            self.status_page.set_icon_name(Some(config::APP_ID));

            // Welcome text which gets displayed when the library is empty. "{}" is the
            // application name.
            self.status_page
                .set_title(&i18n_f("Welcome to {}", &[config::NAME]));

            // Station flowbox
            self.flowbox.init(library.model());

            // Set initial stack page
            self.update_stack_page();

            library.connect_notify_local(
                Some("status"),
                clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |_, _| this.update_stack_page()
                ),
            );
        }
    }

    impl WidgetImpl for SwLibraryPage {}

    impl NavigationPageImpl for SwLibraryPage {}

    impl SwLibraryPage {
        fn update_stack_page(&self) {
            let status = SwApplication::default().library().status();
            match status {
                SwLibraryStatus::Empty => self.stack.set_visible_child_name("empty"),
                SwLibraryStatus::Content => self.stack.set_visible_child_name("content"),
                _ => (),
            }
        }
    }
}

glib::wrapper! {
    pub struct SwLibraryPage(ObjectSubclass<imp::SwLibraryPage>)
        @extends gtk::Widget, adw::NavigationPage;
}
