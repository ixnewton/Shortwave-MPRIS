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

static STATION_SEARCH: &str = "json/stations/search";
static STATS: &str = "json/stats";

pub mod client;
mod cover_loader;
mod error;
mod station;
mod station_metadata;
mod station_model;
mod station_request;
mod station_sorter;
mod stats;

pub use cover_loader::CoverLoader;
pub use error::Error;
pub use station::SwStation;
pub use station_metadata::StationMetadata;
pub use station_model::SwStationModel;
pub use station_request::StationRequest;
pub use station_sorter::{SwStationSorter, SwStationSorting, SwStationSortingType};
pub use stats::Stats;
