// Shortwave - station_row.rs
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

use adw::subclass::prelude::*;
use glib::clone;
use glib::subclass;
use glib::Properties;
use gtk::prelude::*;
use gtk::{glib, CompositeTemplate};
use inflector::Inflector;

use crate::api::StationMetadata;
use crate::api::SwStation;
use crate::ui::SwStationCover;
use crate::SwApplication;
use crate::i18n::i18n;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/station_row.ui")]
    #[properties(wrapper_type = super::SwStationRow)]
    pub struct SwStationRow {
        #[template_child]
        station_label: TemplateChild<gtk::Label>,
        #[template_child]
        subtitle_label: TemplateChild<gtk::Label>,
        #[template_child]
        station_cover: TemplateChild<SwStationCover>,
        #[template_child]
        local_image: TemplateChild<gtk::Image>,
        #[template_child]
        orphaned_image: TemplateChild<gtk::Image>,
        #[template_child]
        play_button: TemplateChild<gtk::Button>,

        #[property(get, set=Self::set_station)]
        station: RefCell<Option<SwStation>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStationRow {
        const NAME: &'static str = "SwStationRow";
        type ParentType = adw::Bin;
        type Type = super::SwStationRow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwStationRow {
        fn constructed(&self) {
            self.parent_constructed();

            // Connect to player state changes to update play button icon
            let player = SwApplication::default().player();
            player.connect_state_notify(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_| imp.update_play_button_icon()
            ));
            
            player.connect_station_notify(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_| imp.update_play_button_icon()
            ));

            self.play_button.connect_clicked(clone!(
                #[weak(rename_to = obj)]
                self.obj(),
                move |_| {
                    glib::spawn_future_local(clone!(
                        #[weak]
                        obj,
                        async move {
                            if let Some(station) = obj.station() {
                                let player = SwApplication::default().player();
                                let current_station = player.station();
                                
                                // If the same station is selected, just toggle playback
                                if let Some(ref current_station) = current_station {
                                    if current_station.uuid() == station.uuid() {
                                        player.toggle_playback().await;
                                        return;
                                    }
                                }
                                
                                // Different station or no station currently selected
                                player.set_station(station).await;
                            }
                        }
                    ));
                }
            ));
        }
    }

    impl WidgetImpl for SwStationRow {}

    impl BinImpl for SwStationRow {}

    impl SwStationRow {
        fn set_station(&self, station: Option<&SwStation>) {
            if let Some(station) = station {
                station.connect_metadata_notify(clone!(
                    #[weak(rename_to = imp)]
                    self,
                    move |s| {
                        imp.set_metadata(s.metadata());
                    }
                ));
                self.set_metadata(station.metadata());
            }

            *self.station.borrow_mut() = station.cloned();
            
            // Update play button icon when station changes
            self.update_play_button_icon();
        }

        fn set_metadata(&self, metadata: StationMetadata) {
            self.station_label.set_text(&metadata.name);
            let mut subtitle = metadata.country.to_title_case();

            if subtitle.is_empty() {
                subtitle = metadata.tags;
            } else if !metadata.tags.is_empty() {
                subtitle = format!("{} · {}", subtitle, metadata.formatted_tags());
            }

            self.subtitle_label.set_text(&subtitle);
            self.subtitle_label.set_visible(!subtitle.is_empty());
        }
        
        fn update_play_button_icon(&self) {
            let player = SwApplication::default().player();
            let current_station = player.station();
            let current_state = player.state();
            
            if let Some(station) = self.station.borrow().as_ref() {
                if let Some(ref current_station) = current_station {
                    // Check if this is the currently playing station
                    if current_station.uuid() == station.uuid() 
                        && current_state == crate::audio::SwPlaybackState::Playing {
                        // Show stop icon for currently playing station
                        self.play_button.set_icon_name("media-playback-stop-symbolic");
                        self.play_button.set_tooltip_text(Some(&i18n("Stop")));
                    } else {
                        // Show play icon for non-playing stations
                        self.play_button.set_icon_name("media-playback-start-symbolic");
                        self.play_button.set_tooltip_text(Some(&i18n("Play")));
                    }
                } else {
                    // No station currently playing, show play icon
                    self.play_button.set_icon_name("media-playback-start-symbolic");
                    self.play_button.set_tooltip_text(Some(&i18n("Play")));
                }
            }
        }
    }
}

glib::wrapper! {
    pub struct SwStationRow(ObjectSubclass<imp::SwStationRow>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SwStationRow {
    pub fn new(station: &SwStation) -> Self {
        glib::Object::builder().property("station", station).build()
    }
}
