// Shortwave - about_window.rs
// Copyright (C) 2021-2023  Felix Häcker <haeckerfelix@gnome.org>
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

use adw::prelude::*;

use crate::config;
use crate::i18n::*;
use crate::ui::SwApplicationWindow;

pub fn show(parent: &SwApplicationWindow) {
    let vcs_tag = format!("Git Commit: {}", config::VCS_TAG);
    let version = match config::PROFILE {
        "development" => format!("{}-devel", config::VERSION),
        _ => config::VERSION.to_string(),
    };

    adw::AboutDialog::builder()
        .application_icon(config::APP_ID)
        .application_name(config::NAME)
        .designers(["Tobias Bernard"])
        .comments(i18n("Listen to internet radio"))
        .copyright("© 2019-2023 Felix Häcker")
        .debug_info(vcs_tag)
        .developer_name("Felix Häcker")
        .developers([
            "Felix Häcker <haeckerfelix@gnome.org>",
            "Maximiliano Sandoval <msandova@gnome.org>",
            "Elias Projahn",
        ])
        .issue_url("https://gitlab.gnome.org/World/Shortwave/-/issues")
        .license_type(gtk::License::Gpl30)
        .translator_credits(i18n("translator-credits"))
        .version(version)
        .website("https://gitlab.gnome.org/World/Shortwave")
        .build()
        .present(parent);
}
