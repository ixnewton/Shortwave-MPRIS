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

use std::cell::RefCell;
use std::rc::Rc;

use adw::subclass::prelude::*;
use gio::SimpleAction;
use glib::{clone, closure, subclass};
use gtk::prelude::*;
use gtk::{gio, glib, CompositeTemplate};

use crate::api::{Error, StationRequest, SwClient};
use crate::i18n::*;
use crate::ui::{SwApplicationWindow, SwStationFlowBox};

mod imp {
    use super::*;

    #[derive(Debug, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/search_page.ui")]
    pub struct SwSearchPage {
        #[template_child]
        stack: TemplateChild<gtk::Stack>,
        #[template_child]
        flowbox: TemplateChild<SwStationFlowBox>,
        #[template_child]
        search_entry: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        sorting_button_content: TemplateChild<adw::ButtonContent>,
        #[template_child]
        results_limit_box: TemplateChild<gtk::Box>,
        #[template_child]
        results_limit_label: TemplateChild<gtk::Label>,
        #[template_child]
        spinner: TemplateChild<gtk::Spinner>,

        search_action_group: gio::SimpleActionGroup,
        client: SwClient,

        station_request: Rc<RefCell<StationRequest>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwSearchPage {
        const NAME: &'static str = "SwSearchPage";
        type ParentType = adw::NavigationPage;
        type Type = super::SwSearchPage;

        fn new() -> Self {
            let search_action_group = gio::SimpleActionGroup::new();
            let station_request = Rc::new(RefCell::new(StationRequest::search_for_name(None, 250)));
            let client = SwClient::new();

            Self {
                stack: TemplateChild::default(),
                flowbox: TemplateChild::default(),
                search_entry: TemplateChild::default(),
                sorting_button_content: TemplateChild::default(),
                results_limit_box: TemplateChild::default(),
                results_limit_label: TemplateChild::default(),
                spinner: TemplateChild::default(),
                search_action_group,
                client,
                station_request,
            }
        }

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
            let obj = self.obj();

            obj.insert_action_group("search", Some(&self.search_action_group));
            let variant = Some(glib::VariantTy::new("s").unwrap());

            let action = SimpleAction::new_stateful("sorting", variant, &"Votes".to_variant());
            self.search_action_group.add_action(&action);
            action.connect_change_state(clone!(
                #[weak(rename_to = this)]
                self,
                move |action, state| {
                    if let Some(state) = state {
                        action.set_state(state);
                        let order = state.str().unwrap();

                        let label = match order {
                            "Name" => i18n("Name"),
                            "Language" => i18n("Language"),
                            "Country" => i18n("Country"),
                            "State" => i18n("State"),
                            "Votes" => i18n("Votes"),
                            "Bitrate" => i18n("Bitrate"),
                            _ => panic!("unknown sorting state change"),
                        };

                        this.sorting_button_content.set_label(&label);

                        // Update station request and redo search
                        let station_request = StationRequest {
                            order: Some(order.to_lowercase()),
                            ..this.station_request.borrow().clone()
                        };
                        *this.station_request.borrow_mut() = station_request;

                        let fut = clone!(
                            #[weak]
                            this,
                            async move {
                                this.update_search().await;
                            }
                        );
                        glib::MainContext::default().spawn_local(fut);
                    }
                }
            ));

            let action = SimpleAction::new_stateful("order", variant, &"Descending".to_variant());

            self.search_action_group.add_action(&action);
            action.connect_change_state(clone!(
                #[weak(rename_to = this)]
                self,
                move |action, state| {
                    if let Some(state) = state {
                        action.set_state(state);

                        let reverse = if state.str().unwrap() == "Ascending" {
                            this.sorting_button_content
                                .set_icon_name("view-sort-ascending-symbolic");
                            false
                        } else {
                            this.sorting_button_content
                                .set_icon_name("view-sort-descending-symbolic");
                            true
                        };

                        // Update station request and redo search
                        let station_request = StationRequest {
                            reverse: Some(reverse),
                            ..this.station_request.borrow().clone()
                        };
                        *this.station_request.borrow_mut() = station_request;

                        let fut = clone!(
                            #[weak]
                            this,
                            async move {
                                this.update_search().await;
                            }
                        );
                        glib::MainContext::default().spawn_local(fut);
                    }
                }
            ));

            // Automatically focus search entry
            obj.connect_map(|this| {
                let imp = this.imp();
                imp.search_entry.grab_focus();
                imp.search_entry.select_region(0, -1);
            });

            // SwClient is ready / has search results
            self.client.connect_local(
                "ready",
                false,
                clone!(
                    #[weak(rename_to = this)]
                    self,
                    #[upgrade_or]
                    None,
                    move |_| {
                        let max_results = this.station_request.borrow().limit.unwrap();
                        let over_max_results = this.client.model().n_items() >= max_results;
                        this.results_limit_box.set_visible(over_max_results);

                        if this.client.model().n_items() == 0 {
                            this.stack.set_visible_child_name("no-results");
                        } else {
                            this.stack.set_visible_child_name("results");
                        }

                        None
                    }
                ),
            );

            // SwClient error
            self.client.connect_closure(
                "error",
                false,
                closure!(|_: SwClient, err: Error| {
                    warn!("Station data could not be received: {}", err.to_string());

                    let text = i18n("Station data could not be received.");
                    SwApplicationWindow::default().show_notification(&text);
                }),
            );

            let max = self.station_request.borrow().limit.unwrap();
            let text = ni18n_f(
                "The number of results is limited to {} item. Try using a more specific search term.",
                "The number of results is limited to {} items. Try using a more specific search term.",
                max,
                &[&max.to_string()],
            );
            self.results_limit_label.set_text(&text);

            self.flowbox.init(self.client.model());
        }
    }

    impl WidgetImpl for SwSearchPage {}

    impl NavigationPageImpl for SwSearchPage {}

    #[gtk::template_callbacks]
    impl SwSearchPage {
        #[template_callback]
        async fn search_changed(&self) {
            let text = self.search_entry.text().to_string();
            let text = if text.is_empty() { None } else { Some(text) };

            let station_request = StationRequest {
                name: text,
                ..self.station_request.borrow().clone()
            };
            *self.station_request.borrow_mut() = station_request;

            self.update_search().await;
        }

        async fn update_search(&self) {
            // Don't search if search entry is empty
            if self.station_request.borrow().name.is_none() {
                self.stack.set_visible_child_name("empty");
                self.spinner.set_spinning(false);
                return;
            }

            self.stack.set_visible_child_name("spinner");
            self.spinner.set_spinning(true);

            let request = self.station_request.borrow().clone();
            debug!("Search for: {:?}", request);
            self.client.send_station_request(request);
        }
    }
}

glib::wrapper! {
    pub struct SwSearchPage(ObjectSubclass<imp::SwSearchPage>)
        @extends gtk::Widget, adw::NavigationPage;
}
