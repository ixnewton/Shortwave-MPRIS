// Shortwave - volume_control.rs
// Copyright (C) 2024  Felix Häcker <haeckerfelix@gnome.org>
//               2022  Emmanuele Bassi (Original Author, Amberol)
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

use adw::subclass::prelude::*;
use glib::clone;
use glib::{subclass::Signal, Properties};
use gtk::{gio, glib, prelude::*, CompositeTemplate};
use once_cell::sync::Lazy;

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate, Properties)]
    #[template(resource = "/de/haeckerfelix/Shortwave/gtk/volume_control.ui")]
    #[properties(wrapper_type = super::SwVolumeControl)]
    pub struct SwVolumeControl {
        #[template_child]
        volume_low_button: TemplateChild<gtk::Button>,
        #[template_child]
        volume_scale: TemplateChild<gtk::Scale>,
        #[template_child]
        volume_high_image: TemplateChild<gtk::Image>,

        #[property(get=Self::volume, set=Self::set_volume, minimum = 0.0, maximum = 1.0, default = 1.0)]
        volume: PhantomData<f64>,
        #[property(get, set=Self::set_toggle_mute)]
        toggle_mute: Cell<bool>,

        prev_volume: Cell<f64>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwVolumeControl {
        const NAME: &'static str = "SwVolumeControl";
        type ParentType = gtk::Widget;
        type Type = super::SwVolumeControl;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);

            klass.set_layout_manager_type::<gtk::BoxLayout>();
            klass.set_css_name("volume");
            klass.set_accessible_role(gtk::AccessibleRole::Group);

            klass.install_property_action("volume.toggle-mute", "toggle-mute");
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwVolumeControl {
        fn constructed(&self) {
            self.parent_constructed();

            let adj = gtk::Adjustment::builder()
                .lower(0.0)
                .upper(1.0)
                .step_increment(0.05)
                .value(1.0)
                .build();
            self.volume_scale.set_adjustment(&adj);

            adj.connect_notify_local(
                Some("value"),
                clone!(
                    #[weak(rename_to = this)]
                    self,
                    move |adj, _| {
                        let value = adj.value();
                        if value == adj.lower() {
                            this.volume_low_button
                                .set_icon_name("audio-volume-muted-symbolic");
                        } else {
                            this.volume_low_button
                                .set_icon_name("audio-volume-low-symbolic");
                        }
                        this.obj().notify_volume();
                        this.obj().emit_by_name::<()>("volume-changed", &[&value]);
                    }
                ),
            );

            let controller = gtk::EventControllerScroll::builder()
                .name("volume-scroll")
                .flags(gtk::EventControllerScrollFlags::VERTICAL)
                .build();

            controller.connect_scroll(clone!(
                #[weak(rename_to = this)]
                self,
                #[upgrade_or_panic]
                move |_, _, dy| {
                    let adj = this.volume_scale.adjustment();
                    let delta = dy * adj.step_increment();
                    let d = (adj.value() - delta).clamp(adj.lower(), adj.upper());
                    adj.set_value(d);
                    glib::Propagation::Stop
                }
            ));
            self.volume_scale.add_controller(controller);
        }

        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
                vec![Signal::builder("volume-changed")
                    .param_types([f64::static_type()])
                    .build()]
            });

            SIGNALS.as_ref()
        }
    }

    impl WidgetImpl for SwVolumeControl {}

    impl SwVolumeControl {
        fn set_toggle_mute(&self, muted: bool) {
            if muted != self.toggle_mute.replace(muted) {
                if muted {
                    let prev_value = self.volume_scale.value();
                    self.prev_volume.replace(prev_value);
                    self.volume_scale.set_value(0.0);
                } else {
                    let prev_value = self.prev_volume.get();
                    self.volume_scale.set_value(prev_value);
                }
                self.obj().notify_toggle_mute();
            }
        }

        pub fn volume(&self) -> f64 {
            self.volume_scale.value()
        }

        pub fn set_volume(&self, value: f64) {
            self.volume_scale.set_value(value);
        }
    }
}

glib::wrapper! {
    pub struct SwVolumeControl(ObjectSubclass<imp::SwVolumeControl>)
        @extends gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl SwVolumeControl {
    pub fn new() -> Self {
        glib::Object::new()
    }
}

impl Default for SwVolumeControl {
    fn default() -> Self {
        Self::new()
    }
}
