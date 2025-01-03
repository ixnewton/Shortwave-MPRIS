// Shortwave - track_model.rs
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

use std::cell::{Cell, RefCell};

use glib::Properties;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use indexmap::map::IndexMap;

use crate::audio::SwTrack;

mod imp {
    use super::*;

    #[derive(Debug, Properties, Default)]
    #[properties(wrapper_type = super::SwTrackModel)]
    pub struct SwTrackModel {
        #[property(get, set)]
        max_count: Cell<u32>,

        pub map: RefCell<IndexMap<String, SwTrack>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwTrackModel {
        const NAME: &'static str = "SwTrackModel";
        type Type = super::SwTrackModel;
        type Interfaces = (gio::ListModel,);
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwTrackModel {}

    impl ListModelImpl for SwTrackModel {
        fn item_type(&self) -> glib::Type {
            SwTrack::static_type()
        }

        fn n_items(&self) -> u32 {
            self.map.borrow().len() as u32
        }

        fn item(&self, position: u32) -> Option<glib::Object> {
            self.map
                .borrow()
                .get_index(position.try_into().unwrap())
                .map(|(_, o)| o.clone().upcast::<glib::Object>())
        }
    }

    impl SwTrackModel {
        pub fn purge_tracks(&self) {
            let removed = {
                let mut map = self.map.borrow_mut();

                if map.len() > self.obj().max_count() as usize {
                    let len: usize = map.split_off((self.obj().max_count()) as usize).len();
                    len
                } else {
                    0
                }
            };

            if removed > 0 {
                self.obj()
                    .items_changed(self.obj().max_count(), removed as u32, 0);
            }
        }
    }
}

glib::wrapper! {
    pub struct SwTrackModel(ObjectSubclass<imp::SwTrackModel>) @implements gio::ListModel;
}

impl SwTrackModel {
    pub fn new() -> Self {
        glib::Object::new()
    }

    // TODO: Check if track already exists in model, and update it if necessary / rearrange position
    pub fn add_track(&self, track: &SwTrack) {
        {
            let mut map = self.imp().map.borrow_mut();
            if map.contains_key(&track.uuid()) {
                warn!("Track {:?} already exists in model", track.title());
                return;
            }

            map.shift_insert(0, track.uuid(), track.clone());
        }

        self.items_changed(0, 0, 1);
        self.imp().purge_tracks();
    }

    pub fn track_by_uuid(&self, uuid: &str) -> Option<SwTrack> {
        self.imp().map.borrow().get(uuid).cloned()
    }
}

impl Default for SwTrackModel {
    fn default() -> Self {
        Self::new()
    }
}
