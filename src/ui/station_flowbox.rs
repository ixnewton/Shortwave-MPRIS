// Shortwave - station_flowbox.rs
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

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{subclass, Properties};
use gtk::{glib, CompositeTemplate};

use crate::api::SwStation;
use crate::model::{SwSorting, SwStationModel, SwStationSorter};
use crate::ui::{SwStationDialog, SwStationRow};

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Debug, CompositeTemplate, Properties)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/station_flowbox.ui")]
    #[properties(wrapper_type = super::SwStationFlowBox)]
    pub struct SwStationFlowBox {
        #[property(get)]
        pub model: gtk::SortListModel,
        #[property(get, set = Self::set_title)]
        pub title: RefCell<Option<String>>,

        #[template_child]
        pub flowbox: TemplateChild<gtk::FlowBox>,

        pub sorter: SwStationSorter,
    }

    impl SwStationFlowBox {
        fn set_title(&self, title: String) {
            self.flowbox
                .update_property(&[gtk::accessible::Property::Label(&title)]);
            self.title.replace(Some(title));
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStationFlowBox {
        const NAME: &'static str = "SwStationFlowBox";
        type ParentType = adw::Bin;
        type Type = super::SwStationFlowBox;

        fn new() -> Self {
            let sorter = SwStationSorter::new();
            let model = gtk::SortListModel::new(None::<SwStationModel>, Some(sorter.clone()));

            Self {
                flowbox: TemplateChild::default(),
                sorter,
                model,
                title: RefCell::default(),
            }
        }

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwStationFlowBox {
        fn constructed(&self) {
            self.flowbox
                .get()
                .bind_model(Some(&self.model), move |station| {
                    let station = station.downcast_ref::<SwStation>().unwrap();
                    let row = SwStationRow::new(station.clone());
                    row.upcast()
                });

            // Show StationDialog when row gets clicked
            self.flowbox.connect_child_activated(move |flowbox, child| {
                let row = child.downcast_ref::<SwStationRow>().unwrap();
                let station = row.station();

                let station_dialog = SwStationDialog::new(&station);
                station_dialog.present(Some(flowbox));
            });
        }
    }

    impl WidgetImpl for SwStationFlowBox {}

    impl BinImpl for SwStationFlowBox {}
}

glib::wrapper! {
    pub struct SwStationFlowBox(ObjectSubclass<imp::SwStationFlowBox>)
        @extends gtk::Widget, adw::Bin;
}

impl SwStationFlowBox {
    pub fn init(&self, model: SwStationModel) {
        let imp = self.imp();
        imp.model.set_model(Some(&model));
    }

    pub fn set_sorting(&self, sorting: SwSorting, descending: bool) {
        let imp = self.imp();
        imp.sorter.set_sorting(sorting);
        imp.sorter.set_descending(descending);
    }
}
