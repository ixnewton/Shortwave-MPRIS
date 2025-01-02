// Shortwave - track.rs
// Copyright (C) 2021-2025  Felix Häcker <haeckerfelix@gnome.org>
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

use super::SwRecordingState;
use crate::api::{Error, SwStation};
use crate::audio::SwPlayer;
use crate::settings::{settings_manager, Key};
use crate::ui::{DisplayError, SwApplicationWindow};

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwTrack)]
    pub struct SwTrack {
        #[property(get)]
        uuid: RefCell<String>,
        #[property(get, set, construct_only)]
        title: OnceCell<String>,
        #[property(get, set, construct_only)]
        station: OnceCell<SwStation>,
        #[property(get)]
        file: OnceCell<gio::File>,
        #[property(get, set, builder(SwRecordingState::default()))]
        state: Cell<SwRecordingState>,
        #[property(get, set)]
        duration: Cell<u64>,

        // Meaningless for SwRecordingMode != "Decide"
        #[property(get, set)]
        save_when_recorded: Cell<bool>,
        #[property(get)]
        #[property(name="is-saved", get=Self::is_saved, type=bool)]
        pub saved_to: RefCell<Option<gio::File>>,

        pub actions: OnceCell<gio::SimpleActionGroup>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwTrack {
        const NAME: &'static str = "SwTrack";
        type Type = super::SwTrack;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwTrack {
        fn constructed(&self) {
            self.parent_constructed();

            // uuid
            let uuid = Uuid::new_v4().to_string();
            *self.uuid.borrow_mut() = uuid;

            // track path
            let mut path = crate::path::DATA.clone();
            path.push("recording");
            path.push(self.obj().uuid().to_string() + ".ogg");

            self.file.set(gio::File::for_path(path)).unwrap();

            // actions
            let actions = gio::SimpleActionGroup::new();

            let cancel_action = gio::SimpleAction::new("cancel", None);
            cancel_action.connect_activate(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_, _| {
                    let player = SwPlayer::default();
                    if let Some(track) = player.playing_track() {
                        if track.uuid() == imp.obj().uuid() {
                            player.cancel_recording();
                        }
                    }
                }
            ));
            cancel_action.set_enabled(false);
            actions.add_action(&cancel_action);

            let save_action = gio::SimpleAction::new("save", None);
            save_action.connect_activate(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_, _| imp.obj().save().handle_error("Unable to save track")
            ));
            save_action.set_enabled(false);
            actions.add_action(&save_action);

            self.obj().connect_state_notify(clone!(
                #[weak]
                save_action,
                #[weak]
                cancel_action,
                move |track| {
                    save_action.set_enabled(track.state() == SwRecordingState::Recorded);
                    cancel_action.set_enabled(track.state() == SwRecordingState::Recording);
                }
            ));

            let play_action = gio::SimpleAction::new("play", None);
            play_action.connect_activate(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_, _| imp.obj().play()
            ));
            play_action.set_enabled(false);
            actions.add_action(&play_action);

            self.obj().connect_is_saved_notify(clone!(
                #[weak]
                play_action,
                move |track| {
                    play_action.set_enabled(track.is_saved());
                }
            ));

            self.actions.set(actions).unwrap();
        }

        fn dispose(&self) {
            if self.obj().state() == SwRecordingState::Recorded {
                self.obj()
                    .file()
                    .delete(gio::Cancellable::NONE)
                    .handle_error("Unable to delete temporary recorded file")
            }
        }
    }

    impl SwTrack {
        fn is_saved(&self) -> bool {
            self.saved_to.borrow().is_some()
        }
    }
}

glib::wrapper! {
    pub struct SwTrack(ObjectSubclass<imp::SwTrack>);
}

impl SwTrack {
    pub fn new(title: &str, station: &SwStation) -> Self {
        glib::Object::builder()
            .property("title", title)
            .property("station", station)
            .build()
    }

    pub fn insert_actions<W: IsA<gtk::Widget>>(&self, widget: &W) {
        widget.insert_action_group("track", Some(self.imp().actions.get().unwrap()));
    }

    pub fn save(&self) -> Result<(), Error> {
        if self.state() != SwRecordingState::Recorded {
            debug!("Track not recorded, not able to save it.");
            return Ok(());
        }

        debug!("Save track \"{}\"", &self.title());

        let directory = settings_manager::string(Key::RecorderSongSavePath);
        let filename = sanitize_filename::sanitize(self.title()) + ".ogg";

        let mut path = PathBuf::from(directory);
        path.push(filename);

        fs::copy(self.file().path().unwrap(), &path).map_err(Rc::new)?;

        *self.imp().saved_to.borrow_mut() = Some(gio::File::for_path(path));
        self.notify_saved_to();
        self.notify_is_saved();

        Ok(())
    }

    pub fn play(&self) {
        if let Some(file) = self.saved_to() {
            debug!("Play track \"{}\"", &self.title());

            let launcher = gtk::FileLauncher::new(Some(&file));
            let window = SwApplicationWindow::default();
            launcher.launch(Some(&window), gio::Cancellable::NONE, |res| {
                res.handle_error("Unable to play track");
            });
        } else {
            debug!("Track not saved, not able to play it.");
        }
    }
}
