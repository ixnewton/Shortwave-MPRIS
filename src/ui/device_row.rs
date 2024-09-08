// Shortwave - device_row.rs
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

use std::cell::OnceCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::clone;
use glib::subclass;
use glib::Properties;
use gtk::prelude::*;
use gtk::{glib, CompositeTemplate};

use crate::audio::SwPlayer;
use crate::device::{SwDevice, SwDeviceKind};
use crate::ui::DisplayError;
use crate::ui::SwDeviceDialog;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/device_row.ui")]
    #[properties(wrapper_type = super::SwDeviceRow)]
    pub struct SwDeviceRow {
        #[template_child]
        pub spinner: TemplateChild<adw::Spinner>,
        #[property(get, set, construct_only)]
        device: OnceCell<SwDevice>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwDeviceRow {
        const NAME: &'static str = "SwDeviceRow";
        type ParentType = adw::ActionRow;
        type Type = super::SwDeviceRow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwDeviceRow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().set_activatable(true);

            let device = self.obj().device();
            device
                .bind_property("name", &*self.obj(), "title")
                .sync_create()
                .build();
            device
                .bind_property("model", &*self.obj(), "subtitle")
                .sync_create()
                .build();

            self.obj().connect_activated(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_| {
                    glib::spawn_future_local(clone!(
                        #[weak]
                        imp,
                        async move {
                            let device = imp.obj().device();
                            let dialog: SwDeviceDialog = imp
                                .obj()
                                .ancestor(SwDeviceDialog::static_type())
                                .unwrap()
                                .downcast()
                                .unwrap();

                            dialog.set_sensitive(false);
                            imp.spinner.set_visible(true);

                            let res = SwPlayer::default().connect_device(&device).await;
                            res.handle_error_in("Unable to connect with device", &dialog);

                            if res.is_ok() {
                                dialog.close();
                            } else {
                                dialog.set_sensitive(true);
                                imp.spinner.set_visible(false);
                            }
                        }
                    ));
                }
            ));
        }
    }

    impl WidgetImpl for SwDeviceRow {}

    impl ListBoxRowImpl for SwDeviceRow {}

    impl PreferencesRowImpl for SwDeviceRow {}

    impl ActionRowImpl for SwDeviceRow {}
}

glib::wrapper! {
    pub struct SwDeviceRow(ObjectSubclass<imp::SwDeviceRow>)
        @extends gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow, adw::ActionRow;
}

impl SwDeviceRow {
    pub fn new(device: &SwDevice) -> Self {
        glib::Object::builder().property("device", device).build()
    }
}
