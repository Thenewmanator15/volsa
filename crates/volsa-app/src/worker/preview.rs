//! A preview of a long series' volume, made from every eighth slice before the volume is
//! filled, so that the 3D view shows something while the rest are decoded. Each of those
//! slices is reduced as it is decoded, so the preview never holds them in full; they are
//! decoded again for the volume, which is filled slab by slab.

use std::sync::Mutex;

use rayon::prelude::*;
use volsa_core::{Grid, HeldVolume, SeriesId};
use volsa_dicom::IndexedSeries;

use super::opened::batches;
use super::volume::Build;
use super::{Shared, lock};
use crate::action::Action;

/// A series of more slices than this is shown as a preview while it is built.
pub const PREVIEW_SLICES: usize = 256;
/// Every this many slices is decoded first, for the preview.
pub const PREVIEW_STRIDE: u32 = 8;
/// Each voxel of the preview is this many of the series' across and down.
pub const PREVIEW_ACROSS: u32 = 4;

/// The slices of a series of `of` decoded first, for its preview: none for a series too
/// short to need one.
pub(super) fn preview_slices(of: usize) -> Vec<usize> {
    if of <= PREVIEW_SLICES {
        return Vec::new();
    }
    (0..of).step_by(PREVIEW_STRIDE as usize).collect()
}

impl Shared {
    /// Make the preview of a series of `held`'s size and place from its slices `of`, keep it
    /// in the store and say so, while the build is still the newest. Nothing when one of
    /// those slices cannot be read.
    pub(super) fn preview_of(
        &self,
        (listed, held): (&IndexedSeries, &HeldVolume),
        of: Vec<usize>,
        build: &Build,
    ) {
        let [cols, rows, _] = held.dims();
        let reduced = Mutex::new(Vec::with_capacity(of.len()));
        batches(listed, of).par_iter().for_each(|file| {
            if !self.building(build) {
                return;
            }
            let image = self.open(&file.entry);
            for (slice, frame) in &file.slices {
                let cached = lock(&self.cache).get(&listed.info.id, *slice);
                let values = cached.map(|frame| frame.values.clone()).or_else(|| {
                    let image = image.as_ref().ok()?;
                    Some(image.decode(*frame).ok()?.values)
                });
                let small = values
                    .map(|values| volsa_volume::reduced(&values, [cols, rows], PREVIEW_ACROSS));
                lock(&reduced).push((u32::try_from(*slice).ok(), small));
            }
        });
        let reduced = reduced
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let slices: Option<Vec<(u32, Vec<f32>)>> = reduced
            .into_iter()
            .map(|(at, values)| Some((at?, values?)))
            .collect();
        let Some(mut slices) = slices else {
            return;
        };
        slices.sort_by_key(|(at, _)| *at);
        let made = volsa_volume::preview_of_reduced(
            held.voxel_to_patient(),
            held.dims(),
            &slices,
            PREVIEW_ACROSS,
        );
        let Some(preview) = made else {
            return;
        };
        self.offer_preview(&listed.info.id, preview, held, build);
    }

    /// Keep a preview in the store and say so, while the build is still the newest.
    fn offer_preview(
        &self,
        series: &SeriesId,
        preview: volsa_core::Volume,
        held: &HeldVolume,
        build: &Build,
    ) {
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
        let first = preview_slices(300);
        assert_eq!(first.len(), 38);
        assert_eq!(&first[..3], [0, 8, 16]);
        assert_eq!(preview_slices(256), Vec::<usize>::new());
    }
}
