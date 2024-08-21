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

use std::cell::{Cell, OnceCell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use adw::prelude::*;
use glib::subclass::prelude::*;
use glib::Properties;
use gtk::glib::Enum;
use gtk::{gio, glib};
use uuid::Uuid;

use crate::api::{Error, SwStation};
use crate::settings::{settings_manager, Key};

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
        #[property(get)]
        uuid: RefCell<String>,
        #[property(get, construct_only)]
        title: OnceCell<String>,
        #[property(get, construct_only)]
        station: OnceCell<SwStation>,
        #[property(get)]
        file: OnceCell<gio::File>,
        #[property(get, set, builder(SwSongState::default()))]
        state: Cell<SwSongState>,
        #[property(get, set)]
        duration: Cell<u64>,
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

            let uuid = Uuid::new_v4().to_string();
            *self.uuid.borrow_mut() = uuid;

            let mut path = crate::path::DATA.clone();
            path.push("recording");
            path.push(self.obj().uuid().to_string() + ".ogg");

            self.file.set(gio::File::for_path(path)).unwrap();
        }

        fn dispose(&self) {
            if let Err(err) = self.obj().file().delete(gio::Cancellable::NONE) {
                error!("Unable to delete recorded file: {}", err.to_string());
            }
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

    pub fn save(&self) -> Result<(), Error> {
        debug!("Save song \"{}\"", &self.title());

        let custom_path = settings_manager::string(Key::RecorderSongSavePath);
        let filename = sanitize_filename::sanitize(self.title()) + ".ogg";

        let path = if !custom_path.is_empty() {
            let mut path = PathBuf::from(custom_path);
            path.push(filename);
            path
        } else {
            // For some unknown reasons some users don't have a xdg-music dir?
            // See: https://gitlab.gnome.org/World/Shortwave/-/issues/676
            let mut path = if let Some(path) = glib::user_special_dir(glib::UserDirectory::Music) {
                path
            } else {
                warn!("Unable to access music directory. Saving song in home directory.");
                glib::home_dir()
            };
            path.push(filename);
            path
        };

        fs::copy(self.file().path().unwrap(), path).map_err(Rc::new)?;
        Ok(())
    }
}
