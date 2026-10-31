//! A preview of a long series' volume, made from every eighth slice as soon as those are in
//! it, so that the 3D view shows something while the rest are decoded.

use voxa_core::{Grid, HeldVolume, SeriesId};

use super::volume::Build;
use super::{Shared, lock};
use crate::action::Action;

/// A series of more slices than this is shown as a preview while it is built.
pub const PREVIEW_SLICES: usize = 256;
/// Every this many slices is decoded first, for the preview.
pub const PREVIEW_STRIDE: u32 = 8;
/// Each voxel of the preview is this many of the series' across and down.
pub const PREVIEW_ACROSS: u32 = 4;

/// The slices of a series of `of` decoded first, for its preview, and those decoded after:
/// none first for a series too short to need a preview.
pub(super) fn passes(of: usize) -> (Vec<usize>, Vec<usize>) {
    if of <= PREVIEW_SLICES {
        return (Vec::new(), (0..of).collect());
    }
    let stride = PREVIEW_STRIDE as usize;
    let first = (0..of).step_by(stride).collect();
    let after = (0..of).filter(|slice| slice % stride != 0).collect();
    (first, after)
}

impl Shared {
    /// Make the preview of a series from its slices `of` already in `held`, keep it in the
    /// store and say so, while the build is still the newest. Nothing when one of those
    /// slices is not there.
    pub(super) fn preview_of(
        &self,
        (held, of): (&HeldVolume, &[usize]),
        series: &SeriesId,
        build: &Build,
    ) {
        let slices: Option<Vec<(u32, Vec<f32>)>> = of
            .iter()
            .map(|&slice| {
                let at = u32::try_from(slice).ok()?;
                Some((at, held.slice_values(at)?))
            })
            .collect();
        let made = slices.and_then(|slices| {
            voxa_volume::preview(
                held.voxel_to_patient(),
                held.dims(),
                &slices,
                PREVIEW_ACROSS,
            )
        });
        let Some(preview) = made else {
            return;
        };
        // Checked with the store locked, so a newer build's preview is never overwritten.
        let mut store = lock(&self.volumes);
        if !self.building(build) {
            return;
        }
        store.put_preview(series.clone(), preview);
        drop(store);
        let corners = held.corners();
        self.report(Action::PreviewReady {
            series: series.clone(),
            corners,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_series_has_every_eighth_slice_decoded_first() {
        let (first, after) = passes(300);
        assert_eq!(first.len(), 38);
        assert_eq!(&first[..3], [0, 8, 16]);
        assert_eq!(first.len() + after.len(), 300);
        assert!(after.iter().all(|slice| slice % 8 != 0));
        assert_eq!(passes(256), (Vec::new(), (0..256).collect()));
    }
}
