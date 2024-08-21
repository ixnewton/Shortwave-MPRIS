// Shortwave - song_model.rs
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

use std::cell::RefCell;

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};
use indexmap::map::IndexMap;

use crate::audio::SwSong;

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct SwSongModel {
        pub map: RefCell<IndexMap<u64, SwSong>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwSongModel {
        const NAME: &'static str = "SwSongModel";
        type Type = super::SwSongModel;
        type Interfaces = (gio::ListModel,);
    }

    impl ObjectImpl for SwSongModel {}

    impl ListModelImpl for SwSongModel {
        fn item_type(&self) -> glib::Type {
            SwSong::static_type()
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
}

glib::wrapper! {
    pub struct SwSongModel(ObjectSubclass<imp::SwSongModel>) @implements gio::ListModel;
}

impl SwSongModel {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn add_song(&self, song: &SwSong) {
        let pos = {
            let mut map = self.imp().map.borrow_mut();
            if map.contains_key(&song.id()) {
                warn!("song {:?} already exists in model", song.title());
                return;
            }

            map.insert(song.id(), song.clone());
            (map.len() - 1) as u32
        };

        self.items_changed(pos, 0, 1);
    }

    pub fn remove_song(&self, song: &SwSong) {
        let mut map = self.imp().map.borrow_mut();

        match map.get_index_of(&song.id()) {
            Some(pos) => {
                map.shift_remove_full(&song.id());
                self.items_changed(pos.try_into().unwrap(), 1, 0);
            }
            None => warn!("song {:?} not found in model", song.title()),
        }
    }
}

impl Default for SwSongModel {
    fn default() -> Self {
        Self::new()
    }
}
