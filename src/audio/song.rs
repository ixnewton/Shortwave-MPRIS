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
use glib::{clone, Properties};
use gtk::{gio, glib};
use uuid::Uuid;

use super::SwSongState;
use crate::api::{Error, SwStation};
use crate::settings::{settings_manager, Key};
use crate::ui::{DisplayError, SwApplicationWindow};

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwSong)]
    pub struct SwSong {
        #[property(get)]
        uuid: RefCell<String>,
        #[property(get, set, construct_only)]
        title: OnceCell<String>,
        #[property(get, set, construct_only)]
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
            if self.obj().state() == SwSongState::Recorded {
                self.obj()
                    .file()
                    .delete(gio::Cancellable::NONE)
                    .handle_error("Unable to delete recorded file")
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

    pub fn insert_actions<W: IsA<gtk::Widget>>(&self, widget: &W) {
        let actions = gio::SimpleActionGroup::new();

        let save = gio::SimpleAction::new("save", None);
        save.connect_activate(clone!(
            #[weak(rename_to = obj)]
            self,
            move |_, _| obj.save().handle_error("Unable to save track")
        ));
        actions.add_action(&save);

        let play = gio::SimpleAction::new("play", None);
        play.connect_activate(clone!(
            #[weak(rename_to = obj)]
            self,
            move |_, _| obj.play()
        ));
        actions.add_action(&play);

        widget.insert_action_group("track", Some(&actions));
    }

    pub fn save(&self) -> Result<(), Error> {
        if self.state() != SwSongState::Recorded {
            debug!("Song not recorded, not able to save it.");
            return Ok(());
        }

        debug!("Save song \"{}\"", &self.title());

        let directory = settings_manager::string(Key::RecorderSongSavePath);
        let filename = sanitize_filename::sanitize(self.title()) + ".ogg";

        let mut path = PathBuf::from(directory);
        path.push(filename);

        fs::copy(self.file().path().unwrap(), path).map_err(Rc::new)?;

        self.set_state(SwSongState::Saved);
        Ok(())
    }

    pub fn play(&self) {
        let launcher = gtk::FileLauncher::new(Some(&self.file()));
        let window = SwApplicationWindow::default();
        launcher.launch(Some(&window), gio::Cancellable::NONE, |res| {
            res.handle_error("Unable to play track");
        });
    }
}
