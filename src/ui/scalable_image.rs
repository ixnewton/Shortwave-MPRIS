// Shortwave - scalable_image.rs
// Copyright (C) 2025  Felix Häcker <haeckerfelix@gnome.org>
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
//
// Ported from Highscore (Alice Mikhaylenko)
// https://gitlab.gnome.org/World/highscore/-/blob/48b3c9ec337e8161997f9d4f2a9ee3e094d5c415/src/widgets/scalable-image.vala

use std::cell::RefCell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use glib::{Properties, clone};
use gtk::graphene::Rect;
use gtk::{gdk, glib};

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwScalableImage)]
    pub struct SwScalableImage {
        #[property(get, set, nullable)]
        texture: RefCell<Option<gdk::Texture>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwScalableImage {
        const NAME: &'static str = "SwScalableImage";
        type ParentType = gtk::Widget;
        type Type = super::SwScalableImage;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwScalableImage {
        fn constructed(&self) {
            self.obj().connect_texture_notify(clone!(
                #[weak(rename_to = imp)]
                self,
                move |_| {
                    imp.obj().queue_draw();
                }
            ));
        }
    }

    impl WidgetImpl for SwScalableImage {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(texture) = self.texture.borrow().clone() {
                let w = self.obj().width();
                let h = self.obj().height();

                snapshot.append_texture(&texture, &Rect::new(0.0, 0.0, w as f32, h as f32));
            }
        }
    }
}

glib::wrapper! {
    pub struct SwScalableImage(ObjectSubclass<imp::SwScalableImage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}
