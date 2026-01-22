// Shortwave - display_error.rs
// Copyright (C) 2021-2025  Felix Häcker <haeckerfelix@gnome.org>
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

// Original source: Key Rack / Sophie Herold
// https://gitlab.gnome.org/sophie-h/key-rack/-/blob/9ca245815be2b81faa8cd028ed6030efe19d6832/src/utils/error.rs

use std::fmt::Display;

use adw::prelude::*;

use crate::app::SwApplication;
use crate::ui::SwApplicationWindow;

pub trait DisplayError<E> {
    fn handle_error(&self, title: impl AsRef<str>);
    fn handle_error_in(&self, title: impl AsRef<str>, window: &SwApplicationWindow);
}

impl<E: Display, T> DisplayError<E> for Result<T, E> {
    fn handle_error(&self, title: impl AsRef<str>) {
        if let Some(window) = SwApplication::default().active_window() {
            let window = window.downcast::<SwApplicationWindow>().unwrap();
            self.handle_error_in(title, &window);
        }
    }

    fn handle_error_in(&self, title: impl AsRef<str>, window: &SwApplicationWindow) {
        if let Err(err) = self {
            error!("{}: {err}", title.as_ref());

            let heading = title.as_ref().to_string();
            let body = err.to_string();
            let transient_for = window.clone();

            // Show alert dialog directly without toast
            let msg = adw::AlertDialog::builder()
                .heading(&heading)
                .body(&body)
                .build();

            msg.add_response("close", "Close");
            msg.present(Some(&transient_for));
        }
    }
}
