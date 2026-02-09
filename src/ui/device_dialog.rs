// Shortwave - device_dialog.rs
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

use std::marker::PhantomData;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{clone, subclass, Properties};
use gtk::{glib, CompositeTemplate};

use crate::app::SwApplication;
use crate::audio::SwPlayer;
use crate::device::SwDevice;
use crate::ui::SwDeviceRow;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/device_dialog.ui")]
    #[properties(wrapper_type = super::SwDeviceDialog)]
    pub struct SwDeviceDialog {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub devices_listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub scan_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub scan_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub scan_spinner: TemplateChild<adw::Spinner>,
        #[template_child]
        pub dialog_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub no_devices_page: TemplateChild<adw::StatusPage>,
        #[template_child]
        pub devices_page: TemplateChild<gtk::ScrolledWindow>,

        #[property(get=Self::player)]
        pub player: PhantomData<SwPlayer>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwDeviceDialog {
        const NAME: &'static str = "SwDeviceDialog";
        type ParentType = adw::Dialog;
        type Type = super::SwDeviceDialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::bind_template_callbacks(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwDeviceDialog {
        fn constructed(&self) {
            self.parent_constructed();
            let player = self.obj().player();

            player
                .device_discovery()
                .devices()
                .connect_items_changed(clone!(
                    #[weak(rename_to = imp)]
                    self,
                    move |_, _, _, _| {
                        imp.update_dialog_stack();
                    }
                ));

            player.device_discovery().connect_is_scanning_notify(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_| {
                    imp.update_scan_stack();
                }
            ));

            self.devices_listbox
                .bind_model(Some(&player.device_discovery().devices()), move |o| {
                    let device: &SwDevice = o.downcast_ref().unwrap();
                    SwDeviceRow::new(device).into()
                });

            self.update_dialog_stack();
            self.update_scan_stack();
            
            // Automatically start device scan when dialog is opened
            glib::spawn_future_local(clone!(
                #[weak(rename_to = imp)]
                self,
                async move {
                    imp.obj().player().device_discovery().scan().await;
                }
            ));
        }
    }

    impl WidgetImpl for SwDeviceDialog {}

    impl AdwDialogImpl for SwDeviceDialog {}

    #[gtk::template_callbacks]
    impl SwDeviceDialog {
        fn player(&self) -> SwPlayer {
            SwApplication::default().player()
        }

        #[template_callback]
        async fn scan(&self) {
            self.obj().player().device_discovery().scan().await;
        }

        fn update_dialog_stack(&self) {
            if self.obj().player().device_discovery().devices().n_items() > 0 {
                self.dialog_stack.set_visible_child(&*self.devices_page);
            } else {
                self.dialog_stack.set_visible_child(&*self.no_devices_page);
            }
        }

        fn update_scan_stack(&self) {
            if self.obj().player().device_discovery().is_scanning() {
                self.scan_stack.set_visible_child(&*self.scan_spinner);
            } else {
                self.scan_stack.set_visible_child(&*self.scan_button);
            }
        }
    }
}

glib::wrapper! {
    pub struct SwDeviceDialog(ObjectSubclass<imp::SwDeviceDialog>)
        @extends gtk::Widget, adw::Dialog,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SwDeviceDialog {
    pub fn new() -> Self {
        glib::Object::new()
    }
}


impl Default for SwDeviceDialog {
    fn default() -> Self {
        Self::new()
    }
}
