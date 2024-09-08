// Shortwave - station_sorter.rs
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

use glib::{Enum, Properties};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use crate::api::SwStation;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwStationSorter)]
    pub struct SwStationSorter {
        #[property(get, set=Self::set_sorting, builder(SwStationSorting::default()))]
        pub sorting: Cell<SwStationSorting>,
        #[property(get, set=Self::set_sorting_type, builder(SwStationSortingType::Ascending))]
        pub sorting_type: Cell<SwStationSortingType>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStationSorter {
        const NAME: &'static str = "SwStationSorter";
        type Type = super::SwStationSorter;
        type ParentType = gtk::Sorter;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwStationSorter {}

    impl SorterImpl for SwStationSorter {
        fn order(&self) -> gtk::SorterOrder {
            gtk::SorterOrder::Total
        }

        fn compare(&self, item1: &glib::Object, item2: &glib::Object) -> gtk::Ordering {
            let a = &item1.clone().downcast::<SwStation>().unwrap();
            let b = &item2.clone().downcast::<SwStation>().unwrap();
            Self::station_cmp(
                a,
                b,
                self.sorting.get(),
                self.sorting_type.get() == SwStationSortingType::Descending,
            )
            .into()
        }
    }

    impl SwStationSorter {
        fn set_sorting(&self, sorting: SwStationSorting) {
            self.sorting.set(sorting);
            self.obj().changed(gtk::SorterChange::Different);
        }

        fn set_sorting_type(&self, sorting_type: SwStationSortingType) {
            self.sorting_type.set(sorting_type);
            self.obj().changed(gtk::SorterChange::Different);
        }

        fn station_cmp(
            a: &SwStation,
            b: &SwStation,
            sorting: SwStationSorting,
            descending: bool,
        ) -> std::cmp::Ordering {
            let mut station_a = a.clone();
            let mut station_b = b.clone();

            if descending {
                std::mem::swap(&mut station_a, &mut station_b);
            }

            match sorting {
                SwStationSorting::Default => std::cmp::Ordering::Equal,
                SwStationSorting::Name => station_a.metadata().name.cmp(&station_b.metadata().name),
                SwStationSorting::Language => station_a
                    .metadata()
                    .language
                    .cmp(&station_b.metadata().language),
                SwStationSorting::Country => station_a
                    .metadata()
                    .country
                    .cmp(&station_b.metadata().country),
                SwStationSorting::State => {
                    station_a.metadata().state.cmp(&station_b.metadata().state)
                }
                SwStationSorting::Codec => {
                    station_a.metadata().codec.cmp(&station_b.metadata().codec)
                }
                SwStationSorting::Votes => {
                    station_a.metadata().votes.cmp(&station_b.metadata().votes)
                }
                SwStationSorting::Bitrate => station_a
                    .metadata()
                    .bitrate
                    .cmp(&station_b.metadata().bitrate),
            }
        }
    }
}

glib::wrapper! {
    pub struct SwStationSorter(ObjectSubclass<imp::SwStationSorter>) @extends gtk::Sorter;
}

impl SwStationSorter {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for SwStationSorter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Display, Copy, Debug, Clone, EnumString, Eq, PartialEq, Enum)]
#[repr(u32)]
#[enum_type(name = "SwStationSorting")]
#[derive(Default)]
pub enum SwStationSorting {
    #[default]
    Default,
    Name,
    Language,
    Country,
    State,
    Codec,
    Votes,
    Bitrate,
}

#[derive(Display, Copy, Debug, Clone, EnumString, Eq, PartialEq, Enum)]
#[repr(u32)]
#[enum_type(name = "SwStationSortingType")]
#[derive(Default)]
pub enum SwStationSortingType {
    #[default]
    Ascending,
    Descending,
}
