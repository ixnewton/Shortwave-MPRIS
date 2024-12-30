// Shortwave - song_state.rs
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
#[enum_type(name = "SwSongState")]
#[derive(Default)]
pub enum SwSongState {
    #[default]
    None,
    Recording,
    Recorded,
    SkippedIncomplete,
    SkippedIgnored,
    BelowThreshold,
    Discarded,
    Saved,
}

impl SwSongState {
    pub fn include_in_history(&self) -> bool {
        *self != Self::SkippedIgnored && *self != Self::BelowThreshold
    }

    pub fn title(&self) -> String {
        match self {
            SwSongState::Recording => i18n("Recording…"),
            SwSongState::SkippedIgnored => i18n("Ignored Track"),
            SwSongState::SkippedIncomplete => i18n("Not Recorded"),
            SwSongState::None => i18n("Not Recorded"),
            SwSongState::Discarded => i18n("Discarded Recording"),
            SwSongState::Recorded => i18n("Recorded"),
            SwSongState::BelowThreshold => i18n("Below Threshold"),
            SwSongState::Saved => i18n("Saved"),
        }
    }

    pub fn description(&self) -> String {
        match self {
            SwSongState::Recording => i18n("Track will be recorded until a new track gets played"),
            SwSongState::SkippedIgnored => i18n("Track contains a word that is on the ignore list"),
            SwSongState::SkippedIncomplete => {
                i18n("Track wasn't played from the beginning, so it can't be fully recorded")
            }
            SwSongState::None => i18n("Recording is deactivated in preferences"),
            SwSongState::Discarded => i18n("Recording was interrupted, recorded data is discarded"),
            SwSongState::Recorded => i18n("Track has been temporarily recorded and can be saved"),
            SwSongState::BelowThreshold => {
                i18n("Track has been discarded as the duration was below the set threshold")
            }
            SwSongState::Saved => i18n("Track was saved in the configured directory"),
        }
    }
}
