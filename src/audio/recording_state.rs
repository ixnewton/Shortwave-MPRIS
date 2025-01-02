// Shortwave - recording_state.rs
// Copyright (C) 2024-2025  Felix Häcker <haeckerfelix@gnome.org>
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

use gtk::glib;
use gtk::glib::Enum;

use crate::i18n::i18n;

#[derive(Display, Copy, Debug, Clone, EnumString, Eq, PartialEq, Enum)]
#[repr(u32)]
#[enum_type(name = "SwRecordingState")]
#[derive(Default)]
pub enum SwRecordingState {
    // Idle
    #[default]
    IdleDisabled,
    IdleIgnoredTrack,
    IdleIncomplete,

    // Recording
    Recording,
    Recorded,

    // Discarded
    DiscardedBelowThreshold,
    DiscardedCancelled,
}

impl SwRecordingState {
    pub fn include_in_past_tracks(&self) -> bool {
        *self != Self::IdleIgnoredTrack && *self != Self::DiscardedBelowThreshold
    }

    pub fn title(&self) -> String {
        match self {
            SwRecordingState::IdleDisabled => i18n("Not Recorded"),
            SwRecordingState::IdleIgnoredTrack => i18n("Ignored Track"),
            SwRecordingState::IdleIncomplete => i18n("Not Recorded"),

            SwRecordingState::Recording => i18n("Recording…"),
            SwRecordingState::Recorded => i18n("Recorded"),

            SwRecordingState::DiscardedBelowThreshold => i18n("Below Threshold"),
            SwRecordingState::DiscardedCancelled => i18n("Cancelled"),
        }
    }

    pub fn description(&self) -> String {
        match self {
            SwRecordingState::IdleDisabled => i18n("Recording is deactivated in preferences"),
            SwRecordingState::IdleIgnoredTrack => {
                i18n("The track contains a word that is on the ignore list")
            }
            SwRecordingState::IdleIncomplete => {
                i18n("The track wasn't played from the beginning, so it can't be fully recorded")
            }
            SwRecordingState::Recording => {
                i18n("The track will be recorded until a new track gets played")
            }
            SwRecordingState::Recorded => i18n("The track has been temporarily recorded"),
            SwRecordingState::DiscardedBelowThreshold => {
                i18n("The track has been discarded as the duration was below the set threshold")
            }
            SwRecordingState::DiscardedCancelled => i18n("Recording has been cancelled"),
        }
    }
}
