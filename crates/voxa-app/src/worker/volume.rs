//! Building the volume of a series for the 3D scene: every slice decoded, then stacked.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;
use voxa_core::SeriesId;
use voxa_dicom::{Frame, ImageFile, IndexedSeries};
use voxa_render::{MAX_VOLUME_BYTES, Prepared};
use voxa_volume::assemble;

use super::{Batch, Shared, batches, lock};
use crate::action::Action;

/// Progress is reported each time this many more slices have been decoded.
const REPORT_EVERY: usize = 8;

/// One request for the volume of a series.
pub(super) struct Build {
    /// Which source was open when the request was made.
    opened: u64,
    /// Which request for a volume it is. Only the newest is carried through.
    number: u64,
}

/// A slice's place in its series, and the slice decoded or why it could not be.
type Decoded = (usize, Result<Arc<Frame>, String>);

impl Shared {
    /// A new request for a volume. Any volume being built for an earlier one is abandoned.
    pub(super) fn next_build(&self) -> Build {
        Build {
            opened: self.opened.load(Ordering::SeqCst),
            number: self.builds.fetch_add(1, Ordering::SeqCst) + 1,
        }
    }

    /// Whether a request for a volume is still the newest, in the source still open.
    pub(super) fn building(&self, build: &Build) -> bool {
        self.opened.load(Ordering::SeqCst) == build.opened
            && self.builds.load(Ordering::SeqCst) == build.number
    }

    /// Decode every slice of a series, taking those already in the frame cache from there,
    /// stack them into a volume, keep it in the store, and report how it went. Nothing is
    /// kept or reported for a request that has been overtaken.
    pub(super) fn build_volume(&self, listed: &IndexedSeries, build: &Build) {
        let series = listed.info.id.clone();
        let of = listed.slices.len();
        let (done, reported) = (AtomicUsize::new(0), Mutex::new(0));
        let one_more = || {
            let done = done.fetch_add(1, Ordering::SeqCst) + 1;
            if done % REPORT_EVERY != 0 || done >= of {
                return;
            }
            // Reported with the lock held, so that a later count is never told first.
            let mut reported = lock(&reported);
            if done > *reported && self.building(build) {
                *reported = done;
                self.report(Action::VolumeProgress {
                    series: series.clone(),
                    done,
                    of,
                });
            }
        };
        let files = batches(listed, (0..of).collect());
        let from_each = |file: &Batch| self.slices_of(file, &series, build, &one_more);
        let mut decoded: Vec<Decoded> = files.par_iter().flat_map_iter(from_each).collect();
        if !self.building(build) {
            return;
        }
        decoded.sort_by_key(|(slice, _)| *slice);
        match stacked(listed, decoded) {
            Ok(volume) => {
                let corners = volume.corners();
                // Made ready for the graphics device here, on a worker, so that the window
                // has only to hand it over.
                let side = self.volume_side.load(Ordering::SeqCst);
                let ready = Prepared::of(&volume, side, MAX_VOLUME_BYTES);
                // Checked with the store locked, so a newer volume is never overwritten.
                let mut store = lock(&self.volumes);
                if !self.building(build) {
                    return;
                }
                store.put(series.clone(), Arc::new(volume));
                store.put_prepared(&series, ready);
                drop(store);
                self.report(Action::VolumeReady { series, corners });
            }
            Err(reason) => self.report(Action::VolumeFailed { series, reason }),
        }
    }

    /// The slices held in one file, each from the frame cache if it is there and decoded
    /// from the file if not. Stops early, with what it has, once the request is overtaken.
    fn slices_of(
        &self,
        file: &Batch,
        series: &SeriesId,
        build: &Build,
        one_more: &impl Fn(),
    ) -> Vec<Decoded> {
        // The file is read only if a slice of it is wanted that the cache does not have.
        let mut opened: Option<Result<ImageFile, String>> = None;
        let mut slices = Vec::with_capacity(file.slices.len());
        for (slice, frame) in &file.slices {
            if !self.building(build) {
                break;
            }
            let cached = lock(&self.cache).get(series, *slice);
            let decoded = cached.map_or_else(
                || match opened.get_or_insert_with(|| self.open(&file.entry)) {
                    Ok(image) => image
                        .decode(*frame)
                        .map(Arc::new)
                        .map_err(|why| why.to_string()),
                    Err(why) => Err(why.clone()),
                },
                Ok,
            );
            slices.push((*slice, decoded));
            one_more();
        }
        slices
    }

    /// Read and parse one file of the source that is open.
    fn open(&self, entry: &str) -> Result<ImageFile, String> {
        let bytes = match lock(&self.source).as_mut() {
            Some(source) => source.read(entry),
            None => return Err("No scan is open.".to_owned()),
        };
        bytes
            .and_then(|bytes| ImageFile::open(&bytes))
            .map_err(|why| why.to_string())
    }
}

/// The decoded slices of a series, given in the series' order, stacked into a volume; or why
/// they cannot be, in plain words, naming the first slice that would not decode.
fn stacked(listed: &IndexedSeries, decoded: Vec<Decoded>) -> Result<voxa_core::Volume, String> {
    let mut frames = Vec::with_capacity(decoded.len());
    for (slice, frame) in decoded {
        let number = slice + 1;
        frames.push(frame.map_err(|why| {
            format!("Slice {number} could not be decoded, so a 3D view cannot be built: {why}")
        })?);
    }
    let values: Vec<&[f32]> = frames.iter().map(|frame| frame.values.as_slice()).collect();
    assemble(&listed.info, &values).map_err(|why| why.to_string())
}
