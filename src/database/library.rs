// Shortwave - library.rs
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

use std::cell::RefCell;

use gtk::{
    gio,
    glib::{self, Object},
    prelude::*,
    subclass::prelude::*,
    Expression,
};

use crate::{
    api::{StationMetadata, SwStation, SwStationModel},
    database::{models::StationEntry, queries, SwLibraryStatus},
};

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct SwLibrary {
        pub model: SwStationModel,
        pub status: RefCell<SwLibraryStatus>,
        pub stations: RefCell<Vec<SwStation>>,
        pub sorted_model: RefCell<Option<gtk::SortListModel>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwLibrary {
        const NAME: &'static str = "SwLibrary";
        type Type = super::SwLibrary;
        type ParentType = Object;
    }

    impl ObjectImpl for SwLibrary {
        fn constructed(&self) {
            self.parent_constructed();

            // Initialize the sorted model
            let list_store = gio::ListStore::new::<SwStation>();
            let sorter = gtk::StringSorter::new(Some(&gtk::PropertyExpression::new(
                SwStation::static_type(),
                None::<&Expression>,
                "title",
            )));
            let sorted_model = gtk::SortListModel::new(Some(list_store), Some(sorter));
            *self.sorted_model.borrow_mut() = Some(sorted_model);

            // Load stations from database
            if let Ok(stations) = queries::stations() {
                let mut station_vec = Vec::new();
                for entry in stations {
                    let data = entry.data.unwrap_or_default();
                    let meta = match serde_json::from_str(&data) {
                        Ok(meta) => meta,
                        Err(_) => StationMetadata::default(),
                    };

                    let station = SwStation::new(
                        &entry.uuid,
                        entry.is_local,
                        meta,
                        None, // No custom cover for now
                    );
                    station_vec.push(station);
                }

                // Add stations to the sorted model
                if let Some(model) = self.sorted_model.borrow().as_ref() {
                    let store = model.model().unwrap().downcast::<gio::ListStore>().unwrap();
                    for station in &station_vec {
                        store.append(station);
                    }
                }

                self.model.add_stations(station_vec);
                self.obj().notify("status");
            }
        }
    }
}

glib::wrapper! {
    pub struct SwLibrary(ObjectSubclass<imp::SwLibrary>);
}

impl Default for SwLibrary {
    fn default() -> Self {
        Object::builder().build()
    }
}

impl SwLibrary {
    pub fn add_station(&self, station: SwStation) {
        let entry = StationEntry::for_station(&station);
        queries::insert_station(entry).unwrap();

        let imp = imp::SwLibrary::from_obj(self);
        imp.stations.borrow_mut().push(station.clone());
        
        // Update the sorted model
        if let Some(model) = imp.sorted_model.borrow().as_ref() {
            let store = model.model().unwrap().downcast::<gio::ListStore>().unwrap();
            store.append(&station);
        }
        
        imp.model.add_stations(vec![station]);
        
        // Update status
        let imp = imp::SwLibrary::from_obj(self);
        if imp.model.n_items() == 0 {
            *imp.status.borrow_mut() = SwLibraryStatus::Empty;
        } else {
            *imp.status.borrow_mut() = SwLibraryStatus::Content;
        }
        self.notify("status");
    }

    pub fn remove_stations(&self, stations: Vec<SwStation>) {
        debug!("Remove {} station(s)", stations.len());
        
        let imp = imp::SwLibrary::from_obj(self);
        let mut stations_list = imp.stations.borrow_mut();
        
        // Remove from internal list
        stations_list.retain(|s| !stations.iter().any(|rs| rs.uuid() == s.uuid()));
        
        // Update the sorted model
        if let Some(model) = imp.sorted_model.borrow().as_ref() {
            let store = model.model().unwrap().downcast::<gio::ListStore>().unwrap();
            for i in (0..store.n_items()).rev() {
                if let Some(item) = store.item(i) {
                    let station = item.downcast::<SwStation>().unwrap();
                    if stations.iter().any(|s| s.uuid() == station.uuid()) {
                        store.remove(i);
                    }
                }
            }
        }

        for station in &stations {
            imp.model.remove_station(station);
            queries::delete_station(&station.uuid()).unwrap();
        }
        
        // Update status
        let imp = imp::SwLibrary::from_obj(self);
        if imp.model.n_items() == 0 {
            *imp.status.borrow_mut() = SwLibraryStatus::Empty;
        } else {
            *imp.status.borrow_mut() = SwLibraryStatus::Content;
        }
        self.notify("status");
    }

    pub fn contains_station(&self, station: &SwStation) -> bool {
        let imp = imp::SwLibrary::from_obj(self);
        imp.stations.borrow().iter().any(|s| s.uuid() == station.uuid())
    }

    pub fn get_next_favorite(&self) -> Option<SwStation> {
        let imp = imp::SwLibrary::from_obj(self);
        if let Some(model) = imp.sorted_model.borrow().as_ref() {
            let n_items = model.n_items();
            if n_items == 0 {
                return None;
            }

            let current_station = crate::app::SwApplication::default().player().station();

            // If no current station, return the first one
            if current_station.is_none() {
                return model
                    .item(0)
                    .and_then(|obj| obj.downcast::<SwStation>().ok());
            }

            let current_station = current_station.unwrap();

            // Find current station index in the sorted model
            for i in 0..n_items {
                if let Some(obj) = model.item(i) {
                    if let Ok(station) = obj.downcast::<SwStation>() {
                        if station.uuid() == current_station.uuid() {
                            // Return next station, or wrap around to first
                            let next_idx = if i + 1 < n_items { i + 1 } else { 0 };
                            return model
                                .item(next_idx)
                                .and_then(|obj| obj.downcast::<SwStation>().ok());
                        }
                    }
                }
            }

            // Current station not found in favorites, return first
            model
                .item(0)
                .and_then(|obj| obj.downcast::<SwStation>().ok())
        } else {
            None
        }
    }

    pub fn get_previous_favorite(&self) -> Option<SwStation> {
        let imp = imp::SwLibrary::from_obj(self);
        if let Some(model) = imp.sorted_model.borrow().as_ref() {
            let n_items = model.n_items();
            if n_items == 0 {
                return None;
            }

            let current_station = crate::app::SwApplication::default().player().station();

            // If no current station, return the last one
            if current_station.is_none() {
                let last_idx = n_items - 1;
                return model
                    .item(last_idx)
                    .and_then(|obj| obj.downcast::<SwStation>().ok());
            }

            let current_station = current_station.unwrap();

            // Find current station index in the sorted model
            for i in 0..n_items {
                if let Some(obj) = model.item(i) {
                    if let Ok(station) = obj.downcast::<SwStation>() {
                        if station.uuid() == current_station.uuid() {
                            // Return previous station, or wrap around to last
                            let prev_idx = if i > 0 { i - 1 } else { n_items - 1 };
                            return model
                                .item(prev_idx)
                                .and_then(|obj| obj.downcast::<SwStation>().ok());
                        }
                    }
                }
            }

            // Current station not found in favorites, return last
            let last_idx = n_items - 1;
            model
                .item(last_idx)
                .and_then(|obj| obj.downcast::<SwStation>().ok())
        } else {
            None
        }
    }

    pub fn sorted_model(&self) -> Option<gtk::SortListModel> {
        let imp = imp::SwLibrary::from_obj(self);
        imp.sorted_model.borrow().clone()
    }

    pub fn model(&self) -> SwStationModel {
        let imp = imp::SwLibrary::from_obj(self);
        imp.model.clone()
    }

    pub fn status(&self) -> SwLibraryStatus {
        let imp = imp::SwLibrary::from_obj(self);
        *imp.status.borrow()
    }

    pub async fn update_data(&self) -> Result<(), crate::api::Error> {
        let mut stations_to_update = Vec::new();
        
        // Collect all non-local stations
        for station in self.model().snapshot() {
            let station: &SwStation = station.downcast_ref().unwrap();
            if !station.is_local() {
                stations_to_update.push(station.clone());
            }
        }

        // Update metadata for each station
        for station in stations_to_update {
            // Just update the station in the database
            let entry = StationEntry::for_station(&station);
            queries::update_station(entry).unwrap();
        }

        Ok(())
    }
}
