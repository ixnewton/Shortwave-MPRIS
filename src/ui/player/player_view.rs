// Shortwave - player_view.rs
// Copyright (C) 2024-2025  Felix Häcker <haeckerfelix@gnome.org>
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
use std::marker::PhantomData;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass, Properties};
use gtk::{glib, CompositeTemplate};

use crate::app::SwApplication;
use crate::audio::SwPlayer;
use crate::audio::SwTrack;
use crate::ui::{
    SwDeviceIndicator, SwRecordingIndicator, SwStationCover, SwTrackRow, SwVolumeControl,
};

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/player_view.ui")]
    #[properties(wrapper_type = super::SwPlayerView)]
    pub struct SwPlayerView {
        #[template_child]
        station_cover: TemplateChild<SwStationCover>,
        #[template_child]
        recording_indicator: TemplateChild<SwRecordingIndicator>,
        #[template_child]
        device_indicator: TemplateChild<SwDeviceIndicator>,
        #[template_child]
        volume_control: TemplateChild<SwVolumeControl>,
        #[template_child]
        past_tracks_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        past_tracks_listbox: TemplateChild<gtk::ListBox>,

        #[property(get, set)]
        pub show_gadget_button: Cell<bool>,
        #[property(get=Self::player)]
        pub player: PhantomData<SwPlayer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwPlayerView {
        const NAME: &'static str = "SwPlayerView";
        type ParentType = adw::Bin;
        type Type = super::SwPlayerView;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwPlayerView {
        fn constructed(&self) {
            self.parent_constructed();
            let player = self.obj().player();

            player
                .bind_property("volume", &*self.volume_control, "volume")
                .sync_create()
                .bidirectional()
                .build();

            self.past_tracks_listbox
                .bind_model(Some(&player.past_tracks()), |track| {
                    SwTrackRow::new(track.clone().downcast::<SwTrack>().unwrap().clone()).into()
                });

            player.past_tracks().connect_items_changed(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_, _, _, _| {
                    imp.update_past_tracks_stack();
                }
            ));

            self.obj().set_show_gadget_button(true);
            self.update_past_tracks_stack();
        }
    }

    impl WidgetImpl for SwPlayerView {}

    impl BinImpl for SwPlayerView {}

    #[gtk::template_callbacks]
    impl SwPlayerView {
        fn update_past_tracks_stack(&self) {
            if self.obj().player().past_tracks().n_items() > 0 {
                self.past_tracks_stack
                    .set_visible_child(&*self.past_tracks_listbox);
            }
        }

        fn player(&self) -> SwPlayer {
            SwApplication::default().player()
        }

        #[template_callback]
        fn recording_indicator_clicked(&self) {
            if let Some(track) = self.obj().player().playing_track() {
                SwApplication::default().show_track_dialog(&track);
            }
        }
    }
}

glib::wrapper! {
    pub struct SwPlayerView(ObjectSubclass<imp::SwPlayerView>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SwPlayerView {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for SwPlayerView {
    fn default() -> Self {
        Self::new()
    }
}
