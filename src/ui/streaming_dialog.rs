// Shortwave - station_dialog.rs
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
use std::net::IpAddr;
use std::rc::Rc;
use std::str::FromStr;

use adw::prelude::*;
use adw::subclass::prelude::*;
use async_channel::{Receiver, Sender};
use glib::{clone, subclass};
use gtk::{gdk, glib, CompositeTemplate};

use crate::audio::{GCastDiscoverer, GCastDiscovererMessage};

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/streaming_dialog.ui")]
    pub struct SwStreamingDialog {
        #[template_child]
        pub row_stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub devices_listbox: TemplateChild<gtk::ListBox>,

        pub gcd: OnceCell<Rc<GCastDiscoverer>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStreamingDialog {
        const NAME: &'static str = "SwStreamingDialog";
        type ParentType = adw::Dialog;
        type Type = super::SwStreamingDialog;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
            Self::Type::bind_template_callbacks(klass);
            klass.add_binding_action(gdk::Key::Escape, gdk::ModifierType::empty(), "window.close");
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for SwStreamingDialog {}

    impl WidgetImpl for SwStreamingDialog {}

    impl AdwDialogImpl for SwStreamingDialog {}
}

glib::wrapper! {
    pub struct SwStreamingDialog(ObjectSubclass<imp::SwStreamingDialog>)
        @extends gtk::Widget, adw::Dialog;
}

#[gtk::template_callbacks]
impl SwStreamingDialog {
    pub fn new() -> Self {
        let dialog = glib::Object::new::<Self>();

        // Setup Google Cast discoverer
        let gcd_t = GCastDiscoverer::new();
        let gcd = Rc::new(gcd_t.0);
        spawn!(clone!(
            #[weak]
            gcd,
            async move {
                let _ = gcd.discover().await;
            }
        ));
        let gcd_receiver = gcd_t.1;

        let imp = dialog.imp();
        imp.gcd.set(gcd).unwrap();

        dialog.setup_signals(gcd_receiver);
        dialog
    }

    fn setup_signals(&self, gcd_receiver: Receiver<GCastDiscovererMessage>) {
        glib::spawn_future_local(clone!(
            #[weak(rename_to = this)]
            self,
            async move {
                while let Ok(message) = gcd_receiver.recv().await {
                    let imp = this.imp();

                    match message {
                        GCastDiscovererMessage::DiscoverStarted => {
                            while let Some(child) = imp.devices_listbox.first_child() {
                                imp.devices_listbox.remove(&child);
                            }
                            imp.devices_listbox.set_visible(false);
                            imp.row_stack.set_visible_child_name("loading");
                        }
                        GCastDiscovererMessage::DiscoverEnded => {
                            if imp.devices_listbox.last_child().is_none() {
                                imp.row_stack.set_visible_child_name("no-devices");
                            } else {
                                imp.row_stack.set_visible_child_name("ready");
                            }
                        }
                        GCastDiscovererMessage::FoundDevice(device) => {
                            imp.row_stack.set_visible_child_name("ready");

                            let row = adw::ActionRow::new();
                            row.set_title(&device.name);
                            row.set_subtitle(&device.ip.to_string());
                            row.set_activatable(true);

                            imp.devices_listbox.append(&row);
                            imp.devices_listbox.set_visible(true);
                        }
                    }
                }
            }
        ));

        self.imp().devices_listbox.connect_row_activated(clone!(
            #[weak(rename_to = this)]
            self,
            move |_, row| {
                let imp = this.imp();
                let row: adw::ActionRow = row.clone().downcast().unwrap();
                let ip_addr: IpAddr = IpAddr::from_str(row.subtitle().unwrap().as_str()).unwrap();

                // Get GCastDevice
                let device = imp.gcd.get().unwrap().device_by_ip_addr(ip_addr).unwrap();

                // TODO:
                //crate::utils::send(
                // TODO: Action::PlaybackConnectGCastDevice(device),
                //);
                this.close();
            }
        ));
    }

    #[template_callback]
    async fn search_again(&self) {
        let _ = self.imp().gcd.get().unwrap().discover().await;
    }
}
