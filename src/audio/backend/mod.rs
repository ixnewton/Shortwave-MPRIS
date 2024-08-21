// Shortwave - mod.rs
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

mod gstreamer_backend;

use async_channel::Receiver;
use gstreamer_backend::GstreamerBackend;
pub use gstreamer_backend::GstreamerChange;

#[derive(Debug)]
pub struct Backend {
    pub gstreamer: GstreamerBackend,
    pub gstreamer_receiver: Option<Receiver<GstreamerChange>>,
}

impl Default for Backend {
    fn default() -> Self {
        // Gstreamer backend
        let (gstreamer_sender, gstreamer_receiver) = async_channel::bounded(10);
        let gstreamer_receiver = Some(gstreamer_receiver);
        let gstreamer = GstreamerBackend::new(gstreamer_sender);

        Self {
            gstreamer,
            gstreamer_receiver,
        }
    }
}
