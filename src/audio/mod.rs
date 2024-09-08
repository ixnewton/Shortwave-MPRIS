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

mod backend;
mod mpris;
mod playback_state;
mod player2;
mod song;
mod song_model;
mod song_state;

pub use backend::GstreamerBackend;
pub use mpris::MprisServer;
pub use playback_state::SwPlaybackState;
pub use player2::SwPlayer;
pub use song::SwSong;
pub use song_model::SwSongModel;
pub use song_state::SwSongState;
