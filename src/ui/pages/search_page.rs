// Shortwave - search_page.rs
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

use std::cell::Cell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass};
use gtk::{glib, CompositeTemplate};
use rand::seq::SliceRandom;
use rand::prelude::IndexedRandom;

use crate::api::{client, Error, StationRequest, SwStation, SwStationModel};
use crate::app::SwApplication;
use crate::ui::{DisplayError, SwStationRow};

mod imp {
    use super::*;

    #[derive(Default, Debug, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/search_page.ui")]
    pub struct SwSearchPage {
        #[template_child]
        search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        stack: TemplateChild<gtk::Stack>,
        #[template_child]
        popular_flowbox: TemplateChild<gtk::FlowBox>,
        #[template_child]
        random_flowbox: TemplateChild<gtk::FlowBox>,
        #[template_child]
        search_gridview: TemplateChild<gtk::GridView>,
        #[template_child]
        failure_statuspage: TemplateChild<adw::StatusPage>,

        popular_model: SwStationModel,
        random_model: SwStationModel,
        search_model: SwStationModel,

        loaded: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwSearchPage {
        const NAME: &'static str = "SwSearchPage";
        type ParentType = adw::NavigationPage;
        type Type = super::SwSearchPage;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwSearchPage {
        fn constructed(&self) {
            // Discover view
            let flowbox_widget_func = |s: &glib::Object| {
                let station: &SwStation = s.downcast_ref().unwrap();
                let row = SwStationRow::new(station);
                let child = gtk::FlowBoxChild::new();
                child.set_child(Some(&row));
                child.into()
            };

            self.popular_flowbox
                .bind_model(Some(&self.popular_model), flowbox_widget_func);
            self.random_flowbox
                .bind_model(Some(&self.random_model), flowbox_widget_func);

            let child_activate_func = |flowbox: &gtk::FlowBox, child: &gtk::FlowBoxChild| {
                let row = child.child().unwrap().downcast::<SwStationRow>().unwrap();
                if let Some(station) = row.station() {
                    // Play the station directly when activated
                    glib::spawn_future_local(async move {
                        let player = SwApplication::default().player();
                        player.set_station(station).await;
                    });
                }
            };

            self.popular_flowbox
                .connect_child_activated(child_activate_func);
            self.random_flowbox
                .connect_child_activated(child_activate_func);

            // Search grid view
            let model = gtk::NoSelection::new(Some(self.search_model.clone()));
            self.search_gridview.set_model(Some(&model));

            self.search_gridview
                .connect_activate(|gv: &gtk::GridView, pos| {
                    let model = gv.model().unwrap();
                    let station = model.item(pos).unwrap().downcast::<SwStation>().unwrap();
                    
                    // Play the station directly when activated
                    glib::spawn_future_local(async move {
                        let player = SwApplication::default().player();
                        player.set_station(station).await;
                    });
                });

            self.stack.set_visible_child_name("spinner");
        }
    }

    impl WidgetImpl for SwSearchPage {
        fn map(&self) {
            self.parent_map();

            if !self.loaded.get() {
                glib::spawn_future_local(clone!(
                    #[weak(rename_to = imp)]
                    self,
                    async move {
                        imp.refresh_discover_page().await;
                    }
                ));
            }
        }
    }

    impl NavigationPageImpl for SwSearchPage {}

    #[gtk::template_callbacks]
    impl SwSearchPage {
        #[template_callback]
        async fn refresh_discover_page(&self) {
            self.stack.set_visible_child_name("spinner");

            match self.load_discover_stations().await {
                Ok(()) => {
                    self.loaded.set(true);
                    self.search_entry.set_sensitive(true);
                    self.stack.set_visible_child_name("discover");
                }
                Err(e) => {
                    self.stack.set_visible_child_name("failure");
                    self.failure_statuspage
                        .set_description(Some(&e.to_string()));
                }
            }
        }

        async fn load_discover_stations(&self) -> Result<(), Error> {
            debug!("Update discover stations...");
            let countrycode = Self::region_code().unwrap_or("GB".into());

            // Popular stations
            let request = StationRequest {
                limit: Some(100),
                order: Some("votes".into()),
                reverse: Some(true),
                countrycode: Some(countrycode.clone()),
                ..Default::default()
            };

            let mut stations = client::station_request(request).await?;

            // Anything more than 50k votes can be considered as botted spam
            stations.retain(|s| s.metadata().votes < 50_000);

            // Randomize the selection to avoid that always the same stations are visible
            let stations = stations
                .choose_multiple(&mut rand::rng(), 12)
                .cloned()
                .collect::<Vec<SwStation>>();

            self.popular_model.clear();
            self.popular_model.add_stations(stations);

            // Random stations
            let request = StationRequest {
                limit: Some(18),
                order: Some("random".into()),
                countrycode: Some(countrycode),
                ..Default::default()
            };

            let stations = client::station_request(request).await?;
            self.random_model.clear();
            self.random_model.add_stations(stations);

            Ok(())
        }

        #[template_callback]
        async fn search_changed(&self) {
            if !self.loaded.get() {
                return;
            }

            let text = self.search_entry.text().trim().to_string();
            let text = if text.is_empty() { None } else { Some(text) };

            // Don't search if search entry is empty
            if text.is_none() {
                self.stack.set_visible_child_name("discover");
                return;
            }

            let request = StationRequest::search_for_name(text, 1000);
            self.stack.set_visible_child_name("spinner");

            debug!("Search for: {:?}", request);
            let res = client::station_request(request).await;
            res.handle_error("Unable to search for stations");

            if let Ok(stations) = res {
                if stations.is_empty() {
                    self.stack.set_visible_child_name("no-results");
                } else {
                    self.stack.set_visible_child_name("results");
                }

                self.search_model.clear();
                self.search_model.add_stations(stations);
            }
        }

        fn region_code() -> Option<String> {
            let locale = sys_locale::get_locale()?;
            let langtag = language_tags::LanguageTag::parse(&locale).ok()?;
            langtag.region().map(|s| s.to_string())
        }
    }
}

glib::wrapper! {
    pub struct SwSearchPage(ObjectSubclass<imp::SwSearchPage>)
        @extends gtk::Widget, adw::NavigationPage,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
