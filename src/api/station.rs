// Shortwave - station.rs
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
use std::cell::OnceCell;
use std::cell::RefCell;
use std::marker::PhantomData;

use glib::Properties;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, glib};

use crate::api::StationMetadata;

mod imp {
    use super::*;

    #[derive(Debug, Default, Properties)]
    #[properties(wrapper_type = super::SwStation)]
    pub struct SwStation {
        #[property(get, set, construct_only)]
        uuid: OnceCell<String>,
        #[property(get, set, construct_only)]
        is_local: OnceCell<bool>,

        #[property(get, set)]
        metadata: RefCell<StationMetadata>,
        #[property(get=Self::title)]
        title: PhantomData<String>,
        #[property(get, set, nullable)]
        favicon: OnceCell<Option<gdk::Texture>>,
        #[property(get, set)]
        is_orphaned: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SwStation {
        const NAME: &'static str = "SwStation";
        type Type = super::SwStation;
    }

    #[glib::derived_properties]
    impl ObjectImpl for SwStation {}

    impl SwStation {
        fn title(&self) -> String {
            self.obj().metadata().name
        }
    }
}

glib::wrapper! {
    pub struct SwStation(ObjectSubclass<imp::SwStation>);
}

impl SwStation {
    pub fn new(
        uuid: &str,
        is_local: bool,
        metadata: StationMetadata,
        favicon: Option<gdk::Texture>,
    ) -> Self {
        glib::Object::builder()
            .property("uuid", uuid)
            .property("is-local", is_local)
            .property("metadata", metadata)
            .property("favicon", favicon)
            .build()
    }

    // We try playing from `url_resolved` first, which is the pre-resolved
    // URL from the API. However, for local stations, we don't do that, so
    // `url_resolved` will be `None`. In that case we just use `url`, which
    // can also be a potential fallback in case the API misses the resolved
    // URL for some reason.
    pub fn stream_url(&self) -> Option<url::Url> {
        self.metadata().url_resolved.or(self.metadata().url)
    }
}
