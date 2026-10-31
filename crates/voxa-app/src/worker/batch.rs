//! Slices to decode, grouped by the file that holds them.

use voxa_dicom::IndexedSeries;

/// Some slices to decode from one file: (place in the series, frame within the file).
pub(super) struct Batch {
    pub(super) entry: String,
    pub(super) slices: Vec<(usize, Option<u32>)>,
}

/// Group slices by the file that holds them, keeping the order they were asked for in: a
/// file is read and parsed once however many of its frames are wanted.
pub(super) fn batches(listed: &IndexedSeries, wanted: Vec<usize>) -> Vec<Batch> {
    let mut batches: Vec<Batch> = Vec::new();
    for slice in wanted {
        let Some(at) = listed.slices.get(slice) else {
            continue;
        };
        match batches.iter_mut().find(|batch| batch.entry == at.entry) {
            Some(batch) => batch.slices.push((slice, at.frame)),
            None => batches.push(Batch {
                entry: at.entry.clone(),
                slices: vec![(slice, at.frame)],
            }),
        }
    }
    batches
}
