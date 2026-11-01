//! The preset windows of a slice, worked out when it is shown when nothing said them as it
//! was decoded: slices read out of a volume, and slices a volume put by in the frame cache.
//! Working them out for every slice decoded for a volume cost more than decoding it did.

use std::borrow::Cow;
use std::sync::Arc;

use volsa_core::SeriesId;
use volsa_volume::PRESETS;

use super::{Workers, lock};
use crate::action::Action;

impl Workers {
    /// Say, from a worker, that a slice about to be shown is ready, with its preset windows,
    /// when nothing has said so: a slice of the series' volume, if that is the one kept, or
    /// one the frame cache holds unannounced. True when the series is held as a volume, so
    /// that nothing of it is to be decoded.
    pub(super) fn say_windows(&self, series: &SeriesId, slice: usize) -> bool {
        let volume = lock(&self.shared.volumes).get(series);
        let held = volume.is_some();
        // Wherever both are locked, the store is locked first; here it is let go first.
        let frame = if held {
            None
        } else {
            lock(&self.shared.cache).take_unannounced(series, slice)
        };
        if volume.is_none() && frame.is_none() {
            return held;
        }
        let (shared, series) = (Arc::clone(&self.shared), series.clone());
        self.spawn(move || {
            let values = match (&volume, &frame) {
                (Some(volume), _) => u32::try_from(slice)
                    .ok()
                    .and_then(|at| volume.slice_values(at))
                    .map(Cow::Owned),
                (None, Some(frame)) => Some(Cow::Borrowed(frame.values.as_slice())),
                (None, None) => None,
            };
            if let Some(values) = values {
                let windows = PRESETS.map(|preset| preset.window(&values));
                shared.report(Action::FrameReady {
                    series,
                    slice,
                    windows,
                });
            }
        });
        held
    }
}

#[cfg(test)]
mod tests;
