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

pub mod pages;
pub mod player;

pub mod about_dialog;
mod add_station_dialog;
mod device_dialog;
mod device_indicator;
mod device_row;
mod display_error;
mod preferences_dialog;
mod recording_indicator;
mod song_row;
mod station_cover;
mod station_dialog;
mod station_row;
mod track_dialog;
mod volume_control;
mod window;

pub use add_station_dialog::SwAddStationDialog;
pub use device_dialog::SwDeviceDialog;
pub use device_indicator::SwDeviceIndicator;
pub use device_row::SwDeviceRow;
pub use display_error::{DisplayError, ToastWindow};
pub use preferences_dialog::SwPreferencesDialog;
pub use recording_indicator::SwRecordingIndicator;
pub use song_row::SwSongRow;
pub use station_cover::SwStationCover;
pub use station_dialog::SwStationDialog;
pub use station_row::SwStationRow;
pub use track_dialog::SwTrackDialog;
pub use volume_control::SwVolumeControl;
pub use window::SwApplicationWindow;
