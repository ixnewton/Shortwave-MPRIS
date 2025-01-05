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
use crate::{i18n::i18n, ui::SwApplicationWindow};

pub trait ToastWindow: IsA<gtk::Widget> {
    fn toast_overlay(&self) -> adw::ToastOverlay;
}

pub trait DisplayError<E> {
    fn handle_error(&self, title: impl AsRef<str>);
    fn handle_error_in(&self, title: impl AsRef<str>, toast_overlay: &impl ToastWindow);
}

impl<E: Display, T> DisplayError<E> for Result<T, E> {
    fn handle_error(&self, title: impl AsRef<str>) {
        let win = SwApplication::default()
            .active_window()
            .unwrap()
            .downcast::<SwApplicationWindow>()
            .unwrap();
        self.handle_error_in(title, &win);
    }

    fn handle_error_in(&self, title: impl AsRef<str>, widget: &impl ToastWindow) {
        if let Err(err) = self {
            error!("{}: {err}", title.as_ref());

            let toast = adw::Toast::builder()
                .title(title.as_ref())
                .button_label(i18n("Show Details"))
                .build();

            let heading = title.as_ref().to_string();
            let body = err.to_string();
            let transient_for = widget.clone();

            toast.connect_local("button-clicked", false, move |_| {
                let msg = adw::AlertDialog::builder()
                    .heading(&heading)
                    .body(&body)
                    .build();

                msg.add_response("close", "Close");
                msg.present(Some(&transient_for));

                None
            });

            widget.toast_overlay().add_toast(toast);
        }
    }
}
