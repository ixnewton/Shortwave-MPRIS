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
mod create_station_dialog;
mod device_dialog;
mod device_indicator;
mod display_error;
mod favicon;
mod featured_carousel;
mod recording_indicator;
mod song_row;
mod station_dialog;
mod station_flowbox;
mod station_row;
mod volume_control;
mod window;

pub use create_station_dialog::SwCreateStationDialog;
pub use device_dialog::SwDeviceDialog;
pub use device_indicator::SwDeviceIndicator;
pub use display_error::{DisplayError, ToastWindow};
pub use favicon::{SwFavicon, SwFaviconSize};
pub use featured_carousel::SwFeaturedCarousel;
pub use recording_indicator::SwRecordingIndicator;
pub use song_row::SwSongRow;
pub use station_dialog::SwStationDialog;
pub use station_flowbox::SwStationFlowBox;
pub use station_row::SwStationRow;
pub use volume_control::SwVolumeControl;
pub use window::SwApplicationWindow;
