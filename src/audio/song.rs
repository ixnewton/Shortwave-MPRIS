// Shortwave - song.rs
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

use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub title: String,
    pub path: PathBuf,
    pub duration: Duration,
}

impl Song {
    pub fn new(title: &str, path: PathBuf, duration: Duration) -> Self {
        debug!("Created new song: \"{}\", {:?}", title, path);

        Self {
            title: title.to_string(),
            path,
            duration,
        }
    }
}

impl PartialEq for Song {
    fn eq(&self, other: &Song) -> bool {
        self.title == other.title
    }
}

use std::cell::{Cell, OnceCell};

use adw::prelude::*;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib::Enum;
use gtk::{gio, glib};

use crate::api::SwStation;

#[derive(Display, Copy, Debug, Clone, EnumString, Eq, PartialEq, Enum)]
#[repr(u32)]
#[enum_type(name = "SwSongState")]
#[derive(Default)]
pub enum SwSongState {
    Recording,
    Recorded,
    #[default]
    Incomplete,
    Ignored,
    BelowThreshold,
    Discarded,
}

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwSong)]
    pub struct SwSong {
        #[property(get, construct_only)]
        title: OnceCell<String>,
        #[property(get, construct_only)]
        station: OnceCell<SwStation>,
        #[property(get)]
        file: OnceCell<gio::File>,
        #[property(get, set, builder(SwSongState::default()))]
        state: Cell<SwSongState>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwSong {
        const NAME: &'static str = "SwSong";
        type Type = super::SwSong;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwSong {
        fn constructed(&self) {
            self.parent_constructed();

            let filename = sanitize_filename::sanitize(self.obj().title() + ".ogg");
            let mut path = crate::path::CACHE.clone();
            path.push("recording");
            path.push(filename);

            self.file.set(gio::File::for_path(path)).unwrap();
        }
    }
}

glib::wrapper! {
    pub struct SwSong(ObjectSubclass<imp::SwSong>);
}

impl SwSong {
    pub fn new(title: &str, station: &SwStation) -> Self {
        glib::Object::builder()
            .property("title", title)
            .property("station", station)
            .build()
    }
}
