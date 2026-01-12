// Shortwave - mod.rs
// Copyright (C) 2024  Felix Häcker <haeckerfelix@gnome.org>
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

mod cast_sender;
mod dlna_sender;
#[allow(clippy::module_inception)]
mod device;
mod device_discovery;
mod device_kind;
mod device_model;

pub use cast_sender::SwCastSender;
pub use device::SwDevice;
pub use device_discovery::SwDeviceDiscovery;
pub use device_kind::SwDeviceKind;
pub use device_model::SwDeviceModel;
pub use dlna_sender::{SwDlnaSender, get_local_ip_for_device};
