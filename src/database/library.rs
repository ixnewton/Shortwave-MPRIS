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
use std::collections::HashMap;

use glib::{Enum, Properties};
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

use super::models::StationEntry;
use crate::api;
use crate::api::StationMetadata;
use crate::api::{SwClient, SwStation};
use crate::database::{connection, queries};
use crate::model::SwStationModel;

#[derive(Display, Copy, Debug, Clone, EnumString, Eq, PartialEq, Enum)]
#[repr(u32)]
#[enum_type(name = "SwLibraryStatus")]
#[derive(Default)]
pub enum SwLibraryStatus {
    #[default]
    Content,
    Empty,
    Offline,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwLibrary)]
    pub struct SwLibrary {
        #[property(get)]
        pub model: SwStationModel,
        #[property(get, builder(SwLibraryStatus::default()))]
        pub status: RefCell<SwLibraryStatus>,

        pub client: SwClient,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwLibrary {
        const NAME: &'static str = "SwLibrary";
        type Type = super::SwLibrary;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwLibrary {
        fn constructed(&self) {
            self.parent_constructed();

            // Load station entries from sqlite database
            let entries = queries::stations().unwrap();
            info!(
                "Loaded {} item(s) from {}",
                entries.len(),
                connection::DB_PATH.to_str().unwrap()
            );

            for entry in entries {
                // Station metadata
                let metadata = if entry.is_local {
                    if let Some(data) = entry.data {
                        match serde_json::from_str(&data) {
                            Ok(m) => m,
                            Err(err) => {
                                error!(
                                    "Unable to deserialize metadata for local station {}: {}",
                                    entry.uuid,
                                    err.to_string()
                                );
                                continue;
                            }
                        }
                    } else {
                        // TODO: Expose error to UI
                        warn!(
                            "No data for local station {}, removing empty entry from database.",
                            entry.uuid
                        );
                        queries::delete_station(&entry.uuid).unwrap();
                        continue;
                    }
                } else if let Some(data) = entry.data {
                    // radio-browser.info station, and we have data cached
                    serde_json::from_str(&data).unwrap_or_default()
                } else {
                    // radio-browser.info station, and we have no data cached yet
                    StationMetadata::default()
                };

                // Station favicon
                let favicon = if let Some(data) = entry.favicon {
                    gtk::gdk::Texture::from_bytes(&glib::Bytes::from_owned(data)).ok()
                } else {
                    None
                };

                let station = SwStation::new(&entry.uuid, entry.is_local, metadata, favicon);
                self.model.add_station(&station);
            }

            self.obj().update_library_status();
        }
    }
}

glib::wrapper! {
    pub struct SwLibrary(ObjectSubclass<imp::SwLibrary>);
}

impl SwLibrary {
    pub async fn update_data(&self) -> Result<(), api::Error> {
        let mut stations_to_update: HashMap<String, SwStation> = HashMap::new();
        let mut uuids_to_update = Vec::new();

        // Collect all relevant UUIDs
        for station in self.model().snapshot() {
            let station: &SwStation = station.downcast_ref().unwrap();
            if !station.is_local() {
                stations_to_update.insert(station.uuid(), station.clone());
                uuids_to_update.push(station.uuid());
            }
        }

        // Retrieve updated station metadata for those UUIDs
        let result = self
            .imp()
            .client
            .clone()
            .station_metadata_by_uuid(uuids_to_update)
            .await?;

        for metadata in result {
            if let Some(station) = stations_to_update.remove(&metadata.stationuuid) {
                station.set_metadata(metadata.clone());
                debug!(
                    "Updated station metadata for {} ({})",
                    station.metadata().name,
                    station.metadata().stationuuid
                );

                // Update cache
                let entry = StationEntry::for_station(&station);
                queries::update_station(entry).unwrap();
            } else {
                warn!(
                    "Unable to update station metadata for {} ({}): Not found in database",
                    metadata.name, metadata.stationuuid
                );
            }
        }

        // Iterate through stations for which we haven't been able to fetch
        // updated metadata from radio-browser.info and mark them as orphaned.
        for (_, station) in stations_to_update {
            debug!(
                "Unable to update station metadata for {} ({}): Station is orphaned",
                station.metadata().name,
                station.metadata().stationuuid
            );
            station.set_is_orphaned(true);
        }

        Ok(())
    }

    pub fn add_stations(&self, stations: Vec<SwStation>) {
        debug!("Add {} station(s)", stations.len());
        for station in stations {
            self.imp().model.add_station(&station);

            let entry = StationEntry::for_station(&station);
            queries::insert_station(entry).unwrap();
        }

        self.update_library_status();
    }

    pub fn remove_stations(&self, stations: Vec<SwStation>) {
        debug!("Remove {} station(s)", stations.len());
        for station in stations {
            self.imp().model.remove_station(&station);
            queries::delete_station(&station.uuid()).unwrap();
        }

        self.update_library_status();
    }

    pub fn contains_station(station: &SwStation) -> bool {
        queries::contains_station(&station.uuid()).unwrap()
    }

    fn update_library_status(&self) {
        let imp = self.imp();

        if imp.model.n_items() == 0 {
            *imp.status.borrow_mut() = SwLibraryStatus::Empty;
        } else {
            *imp.status.borrow_mut() = SwLibraryStatus::Content;
        }

        self.notify_status();
    }
}

impl Default for SwLibrary {
    fn default() -> Self {
        glib::Object::new()
    }
}
