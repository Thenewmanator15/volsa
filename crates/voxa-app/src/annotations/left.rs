//! The measurements of the series the scene has left in this run, with what could be undone on
//! each, so that clearing them, then looking at another scan and coming back, can still be taken
//! back.

use voxa_core::SeriesId;

use super::Annotations;

/// How many series are remembered; the one left longest ago is forgotten first.
pub const MOST: usize = 8;

/// The lists of the series the scene has left, the one left last at the end.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Left {
    lists: Vec<(SeriesId, Annotations)>,
}

impl Left {
    /// Put away the list of a series the scene is leaving. A list with nothing to undo or do
    /// again is just what was read from where it is kept, and is read again when the series is
    /// shown, so it is not put away.
    pub fn put_away(&mut self, series: SeriesId, list: Annotations) {
        if !list.can_undo() && !list.can_redo() {
            return;
        }
        self.lists.retain(|(left, _)| *left != series);
        self.lists.push((series, list));
        if self.lists.len() > MOST {
            self.lists.remove(0);
        }
    }

    /// Take back the list put away for a series the scene has come back to, if one was.
    pub fn take_back(&mut self, series: &SeriesId) -> Option<Annotations> {
        let at = self.lists.iter().position(|(left, _)| left == series)?;
        Some(self.lists.remove(at).1)
    }
}
