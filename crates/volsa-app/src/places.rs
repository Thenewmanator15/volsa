//! Where a person left each scan, so that it opens there the next time: the slice shown, the
//! window if they set one themselves, and how the picture was zoomed, moved, flipped and
//! turned; and, for a study, the series last shown in the first viewport. A place holds
//! numbers alone, and nothing from the scan's files.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use volsa_core::{SeriesId, WindowLevel};
use volsa_render::{MAX_ZOOM, MIN_ZOOM, ViewTransform};

use crate::state::ViewportState;

mod kept;

pub(crate) use kept::{load, save};

/// The most series a place is kept for, and the most studies a first series is kept for.
/// Keeping one more drops the one written longest ago.
pub const MAX_PLACES: usize = 500;
/// The farthest a kept picture may have been moved from the middle of its viewport, in screen
/// pixels: far past any screen, and still a number that can be worked with.
const FARTHEST_PAN: f32 = 1.0e5;

/// Everything that can happen to the places kept: what a worker reports of the file they are
/// kept in, and the word to keep them. It is carried by [`Action::Places`](crate::Action::Places).
#[derive(Debug, Clone, PartialEq)]
pub enum PlacesAction {
    /// What is kept for the series and studies of an export has been read.
    Loaded {
        /// Which export it was read for, as [`AppState::opened`](crate::AppState::opened)
        /// counts them: what was read for one that is no longer open is ignored.
        opened: u64,
        /// The place kept for each of its series that has one.
        places: Vec<(SeriesId, Place)>,
        /// The series last shown in the first viewport, if it was kept and is in the export.
        first: Option<SeriesId>,
    },
    /// Keep the place of every series shown now, where it is not the one kept already.
    Keep,
}

/// Where one series was left.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Place {
    /// The slice shown, counting from 0.
    pub slice: usize,
    /// The window's centre and width, if the person set one; `None` while it was Volsa's own
    /// choice, which is made again from the pictures.
    pub window: Option<[f32; 2]>,
    /// How many times larger than fitted the picture was shown.
    pub zoom: f32,
    /// How far the picture was moved from the middle of its viewport, in screen pixels.
    pub pan: [f32; 2],
    /// Whether it was mirrored left to right.
    pub flip_h: bool,
    /// Whether it was mirrored top to bottom.
    pub flip_v: bool,
    /// Quarter turns clockwise, 0 to 3.
    pub turns: u8,
}

impl Place {
    /// Where a viewport is now. What is not a number is written as the number Volsa starts
    /// with, since the file has no way to write it and one such would spoil the whole file.
    pub fn of(viewport: &ViewportState) -> Self {
        let view = viewport.view;
        let finite = |value: f32, otherwise: f32| if value.is_finite() { value } else { otherwise };
        let window = [viewport.wl.center, viewport.wl.width];
        let by_hand = !viewport.wl_auto && window.iter().all(|part| part.is_finite());
        Self {
            slice: viewport.slice,
            window: by_hand.then_some(window),
            zoom: finite(view.zoom, 1.0),
            pan: view.pan.map(|shift| finite(shift, 0.0)),
            flip_h: view.flip_h,
            flip_v: view.flip_v,
            turns: view.quarter_turns % 4,
        }
    }

    /// Where a series of this many slices is when it has just been chosen: its middle slice,
    /// fitted, in Volsa's own window.
    pub fn fresh(slices: usize) -> Self {
        Self::of(&ViewportState {
            slice: slices / 2,
            ..ViewportState::default()
        })
    }

    /// This place as it fits a series of this many slices: a slice past the last becomes the
    /// last. `None` for a place that cannot be one of that series, which is then ignored: a
    /// series of no slices, a number that is not a number, a zoom Volsa does not go to, a
    /// picture moved out of all reach, or more than three quarter turns. A place read from a
    /// file is only numbers someone could have changed, and is trusted no further than this.
    pub fn fitted(self, slices: usize) -> Option<Self> {
        let last = slices.checked_sub(1)?;
        let window = match self.window {
            Some(window) if window.iter().all(|part| part.is_finite()) => {
                let [center, width] = window;
                let held = WindowLevel::new(center, width);
                Some([held.center, held.width])
            }
            Some(_) => return None,
            None => None,
        };
        let within = |shift: &f32| shift.is_finite() && shift.abs() <= FARTHEST_PAN;
        let sound = (MIN_ZOOM..=MAX_ZOOM).contains(&self.zoom)
            && self.pan.iter().all(within)
            && self.turns < 4;
        sound.then_some(Self {
            slice: self.slice.min(last),
            window,
            ..self
        })
    }

    /// How the picture was zoomed, moved, flipped and turned.
    pub fn view(&self) -> ViewTransform {
        ViewTransform {
            zoom: self.zoom,
            pan: self.pan,
            flip_h: self.flip_h,
            flip_v: self.flip_v,
            quarter_turns: self.turns,
        }
    }

    /// The window the person had set, if they had set one.
    pub fn window(&self) -> Option<WindowLevel> {
        self.window
            .map(|[center, width]| WindowLevel::new(center, width))
    }
}

/// What the state knows of the places kept for the export that is open.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Places {
    /// Whether what is kept for this export has been read. Until it has, nothing is written
    /// where the places are kept: what is there would be written over unread.
    pub read: bool,
    /// The place the file holds for each series of this export that has one: as it was read,
    /// and then as it was last asked to be written.
    pub kept: BTreeMap<SeriesId, Place>,
    /// The series the file holds as the one last shown in the first viewport.
    pub first: Option<SeriesId>,
    /// The series Volsa itself chose to open first, until the person chooses one: only its
    /// own choice gives way to the series kept.
    pub automatic: Option<SeriesId>,
}

#[cfg(test)]
mod tests;
