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

pub trait ToastWindow: IsA<gtk::Widget> {
    fn toast_overlay(&self) -> adw::ToastOverlay;
}

pub trait DisplayError<E> {
    fn handle_error(&self, title: impl AsRef<str>);
    fn handle_error_in(&self, title: impl AsRef<str>, toast_overlay: &impl ToastWindow);
}

impl<E: Display, T> DisplayError<E> for Result<T, E> {
    fn handle_error(&self, title: impl AsRef<str>) {
        if let Some(window) = SwApplication::default().active_window() {
            let window = window.downcast::<SwApplicationWindow>().unwrap();
            self.handle_error_in(title, &window);
        }
    }

    fn handle_error_in(&self, title: impl AsRef<str>, widget: &impl ToastWindow) {
        if let Err(err) = self {
            error!("{}: {err}", title.as_ref());

            // Create a frame with rounded corners as the container
            let frame = gtk::Frame::builder()
                .build();
            
            // Create horizontal box for header with close button
            let header_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .build();
            
            // Create bold title label (same size as content)
            let title_label = gtk::Label::builder()
                .label(title.as_ref())
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .max_width_chars(50)
                .xalign(0.0)
                .hexpand(true)
                .build();
            title_label.add_css_class("heading");

            // Create close button
            let close_button = gtk::Button::builder()
                .icon_name("window-close-symbolic")
                .valign(gtk::Align::Start)
                .build();
            close_button.add_css_class("flat");
            close_button.add_css_class("circular");

            header_box.append(&title_label);
            header_box.append(&close_button);
            
            // Create a vertical box to hold header and content
            let vbox = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(6)
                .margin_top(12)
                .margin_bottom(12)
                .margin_start(16)
                .margin_end(16)
                .build();

            // Create regular content label
            let content_label = gtk::Label::builder()
                .label(&err.to_string())
                .wrap(true)
                .wrap_mode(gtk::pango::WrapMode::WordChar)
                .max_width_chars(50)
                .xalign(0.0)
                .build();

            vbox.append(&header_box);
            vbox.append(&content_label);
            frame.set_child(Some(&vbox));

            // Use Toast just as a container for positioning, with our custom frame
            let toast = adw::Toast::builder()
                .custom_title(&frame)
                .timeout(0)
                .build();

            // Connect close button to dismiss toast
            let toast_clone = toast.clone();
            close_button.connect_clicked(move |_| {
                toast_clone.dismiss();
            });

            widget.toast_overlay().add_toast(toast);
        }
    }
}
