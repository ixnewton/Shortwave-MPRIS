// Shortwave - track_state.rs
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

use gtk::glib;
use gtk::glib::Enum;

use crate::i18n::i18n;

// TODO: Rename to SwRecordingState
#[derive(Display, Copy, Debug, Clone, EnumString, Eq, PartialEq, Enum)]
#[repr(u32)]
#[enum_type(name = "SwTrackState")]
#[derive(Default)]
pub enum SwTrackState {
    #[default]
    None,
    Recording,
    Recorded,
    SkippedIncomplete,
    SkippedIgnored,
    BelowThreshold,
    Cancelled,
    Saved,
}

impl SwTrackState {
    pub fn include_in_history(&self) -> bool {
        *self != Self::SkippedIgnored && *self != Self::BelowThreshold
    }

    pub fn title(&self) -> String {
        match self {
            SwTrackState::Recording => i18n("Recording…"),
            SwTrackState::SkippedIgnored => i18n("Ignored Track"),
            SwTrackState::SkippedIncomplete => i18n("Not Recorded"),
            SwTrackState::None => i18n("Not Recorded"),
            SwTrackState::Cancelled => i18n("Cancelled Recording"),
            SwTrackState::Recorded => i18n("Recorded"),
            SwTrackState::BelowThreshold => i18n("Below Threshold"),
            SwTrackState::Saved => i18n("Saved"),
        }
    }

    pub fn description(&self) -> String {
        match self {
            SwTrackState::Recording => {
                i18n("The track will be recorded until a new track gets played")
            }
            SwTrackState::SkippedIgnored => {
                i18n("The track contains a word that is on the ignore list")
            }
            SwTrackState::SkippedIncomplete => {
                i18n("The track wasn't played from the beginning, so it can't be fully recorded")
            }
            SwTrackState::None => i18n("Recording is deactivated in preferences"),
            SwTrackState::Cancelled => {
                i18n("Recording has been cancelled, recorded data is discarded")
            }
            SwTrackState::Recorded => {
                i18n("The track has been temporarily recorded and can be saved")
            }
            SwTrackState::BelowThreshold => {
                i18n("The track has been discarded as the duration was below the set threshold")
            }
            SwTrackState::Saved => i18n("The track was saved in the configured directory"),
        }
    }
}
