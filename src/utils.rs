// Shortwave - utils.rs
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

use ashpd::desktop::background::Background;
use gtk::glib;

use crate::i18n::{i18n, ni18n_f};

pub fn send<T: 'static>(sender: &async_channel::Sender<T>, message: T) {
    let fut = glib::clone!(
        #[strong]
        sender,
        async move {
            if let Err(err) = sender.send(message).await {
                error!(
                    "Failed to send \"{}\" action due to {err}",
                    stringify!(message),
                );
            }
        }
    );
    glib::spawn_future_local(fut);
}

pub fn format_duration(d: u64, short: bool) -> String {
    if short {
        let dt = glib::DateTime::from_unix_local(d.try_into().unwrap_or_default()).unwrap();
        dt.format("%M:%S").unwrap_or_default().to_string()
    } else {
        let time: u32 = d.try_into().unwrap();
        let sec = time % 60;
        let time = time - sec;
        let min = (time % (60 * 60)) / 60;
        let time = time - (min * 60);
        let hour = time / (60 * 60);

        let hours = if hour != 0 {
            ni18n_f("{} hour", "{} hours", hour, &[&hour.to_string()])
        } else {
            String::new()
        };

        let mins = if min != 0 {
            ni18n_f("{} minute", "{} minutes", min, &[&min.to_string()])
        } else {
            String::new()
        };

        let secs = if sec != 0 {
            ni18n_f("{} second", "{} seconds", sec, &[&sec.to_string()])
        } else {
            String::new()
        };

        if hour > 0 {
            format!("{hours} {mins} {secs}")
        } else if min > 0 {
            format!("{mins} {secs}")
        } else if sec > 0 {
            secs
        } else {
            i18n("0 seconds")
        }
    }
}

/// Ellipsizes a string at the end so that it is `max_len` characters long
/// Source: https://gitlab.gnome.org/World/pika-backup/-/blob/6bd7d0df56479ee769a249b466d5ac226f88056b/src/ui/utils.rs#L344
pub fn ellipsize_end<S: std::fmt::Display>(x: S, max_len: usize) -> String {
    let mut text = x.to_string();

    if text.len() <= max_len {
        text
    } else {
        text.truncate(max_len.max(1) - 1);
        text.push('…');
        text
    }
}

pub async fn background_portal_permissions() -> bool {
    if !ashpd::is_sandboxed().await {
        debug!("App is not sandboxed, background playback is allowed.");
        return true;
    }

    if let Ok(res) = Background::request()
        .reason("Play radio station in the background")
        .send()
        .await
    {
        match res.response() {
            Ok(response) => response.run_in_background(),
            Err(err) => {
                warn!("{}", err.to_string());
                false
            }
        }
    } else {
        warn!("Unable to check background permissions, falling back to true.");
        true
    }
}
