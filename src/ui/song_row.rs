// Shortwave - song_row.rs
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

use std::cell::OnceCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass, Properties};
use gtk::{gio, glib, CompositeTemplate};

use crate::audio::SwSong;
use crate::ui::SwApplicationWindow;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[properties(wrapper_type = super::SwSongRow)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/song_row.ui")]
    pub struct SwSongRow {
        #[template_child]
        pub save_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub open_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub button_stack: TemplateChild<gtk::Stack>,

        #[property(get, set, construct_only)]
        pub song: OnceCell<SwSong>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwSongRow {
        const NAME: &'static str = "SwSongRow";
        type ParentType = adw::ActionRow;
        type Type = super::SwSongRow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwSongRow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            let dt = glib::DateTime::from_unix_local(obj.song().duration() as i64).unwrap();
            let duration = dt.format("%M:%S").unwrap_or_default().to_string();

            obj.set_title(&obj.song().title());
            obj.set_tooltip_text(Some(&obj.song().title()));
            obj.set_subtitle(&duration);

            self.save_button.connect_clicked(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_| {
                    if let Err(err) = imp.obj().song().save() {
                        error!("Unable to save song: {}", err.to_string());
                    } else {
                        // Display play button instead of save button
                        imp.button_stack.set_visible_child_name("open");
                        imp.obj()
                            .set_activatable_widget(Some(&imp.open_button.get()));
                    }
                }
            ));

            self.open_button.connect_clicked(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_| {
                    let file = imp.obj().song().file();
                    let launcher = gtk::FileLauncher::new(Some(&file));
                    let window = SwApplicationWindow::default();
                    launcher.launch(Some(&window), gio::Cancellable::NONE, |res| {
                        if let Err(err) = res {
                            error!("Could not open dir: {err}");
                        }
                    });
                }
            ));
        }
    }

    impl WidgetImpl for SwSongRow {}

    impl ListBoxRowImpl for SwSongRow {}

    impl PreferencesRowImpl for SwSongRow {}

    impl ActionRowImpl for SwSongRow {}
}

glib::wrapper! {
    pub struct SwSongRow(ObjectSubclass<imp::SwSongRow>)
        @extends gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow, adw::ActionRow;
}

impl SwSongRow {
    pub fn new(song: SwSong) -> Self {
        glib::Object::builder().property("song", &song).build()
    }
}
