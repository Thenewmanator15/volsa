//! Building the volume of a series held as its values are stored: every slice decoded
//! straight into its place on the workers' threads, with no list of decoded slices and no
//! stacking. How the values are held is decided from the first slice decoded; a later slice
//! stored another way makes the build start again holding 32-bit values.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rayon::prelude::*;
use voxa_core::{HeldError, HeldVolume, Rescale, SeriesId, SliceIn, Stored};
use voxa_dicom::{ImageFile, IndexedSeries, StoredSamples};
use voxa_volume::{VolumeError, can_be_volume};

use super::opened::batches;
use super::preview::preview_slices;
use super::volume::Build;
use super::{Batch, Shared, lock};
use crate::action::Action;

/// The most bytes in one slab of a held volume.
pub(super) const SLAB_BYTES: usize = 32 << 20;
/// Progress is reported each time this many more slices have been decoded.
const REPORT_EVERY: usize = 8;

/// How the build of a volume ended.
pub(super) enum Built {
    /// Every slice is in it.
    Whole(HeldVolume),
    /// It could not be built, for the reason given; what was decoded of it, if anything.
    Failed(Option<HeldVolume>, String),
    /// A newer request overtook it; what was decoded of it.
    Overtaken(HeldVolume),
}

/// What became of one slice.
enum Put {
    Done,
    /// It is stored another way than the volume holds its values.
    Other,
    /// It could not be put, for the reason given in a sentence.
    Failed(String),
}

impl Shared {
    /// Decode every slice of a series into a held volume, taking those already in the frame
    /// cache from there where their stored numbers can be had back exactly, with progress
    /// reported on the way. Stops early, with what it has, once the request is overtaken.
    pub(super) fn build_held(&self, listed: &IndexedSeries, build: &Build) -> Built {
        if let Err(why) = can_be_volume(&listed.info) {
            return Built::Failed(None, why.to_string());
        }
        self.told_no_room
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let (stored, rescale) = self.first_kind(listed);
        let voxels = listed.info.voxel_count();
        let values =
            usize::try_from(voxels).map_or(usize::MAX, |n| n.saturating_mul(stored.bytes()));
        if let Some(why) = self.too_little_memory(values) {
            return Built::Failed(None, why);
        }
        if let Some(built) = self.fill(listed, build, stored, rescale) {
            return built;
        }
        let float = (Stored::Float32, Rescale::default());
        self.fill(listed, build, float.0, float.1)
            .unwrap_or_else(|| Built::Failed(None, VolumeError::NoGeometry.to_string()))
    }

    /// How the series' values are to be held: as the first slice not in the frame cache is
    /// stored, or 32-bit values when it cannot be read or held in 16 bits exactly.
    fn first_kind(&self, listed: &IndexedSeries) -> (Stored, Rescale) {
        let series = &listed.info.id;
        let cache = lock(&self.cache);
        let first = (0..listed.slices.len()).find(|slice| !cache.contains(series, *slice));
        drop(cache);
        let float = (Stored::Float32, Rescale::default());
        let Some(slice) = listed.slices.get(first.unwrap_or(0)) else {
            return float;
        };
        let stored = self.open(&slice.entry).and_then(|file| {
            file.decode_stored(slice.frame)
                .map_err(|why| why.to_string())
        });
        match stored.map(|frame| frame.samples) {
            Ok(StoredSamples::Unsigned(_, rescale)) => (Stored::Unsigned16, rescale),
            Ok(StoredSamples::Signed(_, rescale)) => (Stored::Signed16, rescale),
            _ => float,
        }
    }

    /// Fill a volume holding its values as `stored`. `None` when a slice is stored another
    /// way, so that the build is to start again in 32 bits.
    fn fill(
        &self,
        listed: &IndexedSeries,
        build: &Build,
        stored: Stored,
        rescale: Rescale,
    ) -> Option<Built> {
        let info = &listed.info;
        let of = listed.slices.len();
        let made = info.voxel_to_patient.and_then(|placed| {
            let dims = [info.cols, info.rows, u32::try_from(of).ok()?];
            HeldVolume::new(dims, placed, stored, rescale, self.slab_bytes())
        });
        let Some(held) = made else {
            return Some(Built::Failed(None, VolumeError::NoGeometry.to_string()));
        };
        let filling = Filling {
            held: &held,
            build,
            other: AtomicBool::new(false),
            failed: Mutex::new(Vec::new()),
            progress: Progress::new(&info.id, of),
        };
        // A long series' every eighth slice first, for a preview while the rest are decoded.
        let first = preview_slices(of);
        if !first.is_empty() {
            self.preview_of((listed, &held), first, build);
        }
        // Then slab by slab, so that each is finished, and past the budget put on disk,
        // before the next is begun.
        for slab in 0..held.slab_count() {
            if !filling.going(self) {
                break;
            }
            let slices = held.slab_slices(slab).map(|slice| slice as usize).collect();
            self.fill_slices(listed, slices, &filling);
            self.keep_within(&held, true);
        }
        let Filling { other, failed, .. } = filling;
        if other.load(Ordering::SeqCst) {
            return None;
        }
        if !self.building(build) {
            return Some(Built::Overtaken(held));
        }
        let first = lock(&failed)
            .iter()
            .min_by_key(|(slice, _)| *slice)
            .cloned();
        Some(match first {
            Some((_, why)) => Built::Failed(Some(held), why),
            None => Built::Whole(held),
        })
    }

    /// Put these slices of a series, on the workers' threads, while the build goes on.
    fn fill_slices(&self, listed: &IndexedSeries, slices: Vec<usize>, filling: &Filling<'_>) {
        let files = batches(listed, slices);
        files.par_iter().for_each(|file| {
            let ended = |put: Put, slice: usize| match put {
                Put::Done => filling.progress.one_more(self, filling.build),
                Put::Other => filling.other.store(true, Ordering::SeqCst),
                Put::Failed(why) => lock(&filling.failed).push((slice, why)),
            };
            let going = || filling.going(self);
            self.fill_file(file, filling.held, &listed.info.id, (&going, &ended));
        });
    }

    /// Put the slices held in one file, each from the frame cache if it is there and decoded
    /// from the file if not, while `going` holds; `ended` is told how each went.
    fn fill_file(
        &self,
        file: &Batch,
        held: &HeldVolume,
        series: &SeriesId,
        (going, ended): (&dyn Fn() -> bool, &dyn Fn(Put, usize)),
    ) {
        // The file is read only if a slice of it is wanted that the cache cannot give.
        let mut opened: Option<Result<ImageFile, String>> = None;
        for (slice, frame) in &file.slices {
            if !going() {
                return;
            }
            let cached = lock(&self.cache).get(series, *slice);
            let at = u32::try_from(*slice).unwrap_or(u32::MAX);
            let put = cached
                .and_then(|cached| from_values(held, at, &cached.values))
                .unwrap_or_else(|| {
                    let image = opened.get_or_insert_with(|| self.open(&file.entry));
                    decoded_into(held, at, image, *frame)
                });
            ended(put, *slice);
        }
    }
}

/// Put a slice of values, read before its stored numbers were known, in a held volume.
/// `None` when the volume holds whole numbers and these values are not all exactly some.
fn from_values(held: &HeldVolume, slice: u32, values: &[f32]) -> Option<Put> {
    let rescale = held.rescale();
    let put = match held.stored() {
        Stored::Float32 => held.put_slice(slice, SliceIn::Float(values)),
        Stored::Unsigned16 => {
            let numbers: Option<Vec<u16>> = values
                .iter()
                .map(|value| u16::try_from(rescale.stored_for(*value)?).ok())
                .collect();
            held.put_slice(slice, SliceIn::Unsigned(&numbers?))
        }
        Stored::Signed16 => {
            let numbers: Option<Vec<i16>> = values
                .iter()
                .map(|value| i16::try_from(rescale.stored_for(*value)?).ok())
                .collect();
            held.put_slice(slice, SliceIn::Signed(&numbers?))
        }
    };
    Some(outcome(slice, put))
}

/// Decode one slice from a file that was read and put it in a held volume.
fn decoded_into(
    held: &HeldVolume,
    slice: u32,
    image: &Result<ImageFile, String>,
    frame: Option<u32>,
) -> Put {
    let decoded = match image {
        Ok(image) => image.decode_stored(frame).map_err(|why| why.to_string()),
        Err(why) => Err(why.clone()),
    };
    let decoded = match decoded {
        Ok(decoded) => decoded,
        Err(why) => {
            let number = slice + 1;
            let said = "could not be decoded, so a 3D view cannot be built";
            return Put::Failed(format!("Slice {number} {said}: {why}"));
        }
    };
    let same = |rescale: &Rescale| *rescale == held.rescale();
    let put = match (&decoded.samples, held.stored()) {
        (StoredSamples::Unsigned(numbers, rescale), Stored::Unsigned16) if same(rescale) => {
            held.put_slice(slice, SliceIn::Unsigned(numbers))
        }
        (StoredSamples::Signed(numbers, rescale), Stored::Signed16) if same(rescale) => {
            held.put_slice(slice, SliceIn::Signed(numbers))
        }
        (_, Stored::Float32) => held.put_slice(slice, SliceIn::Float(&decoded.values())),
        _ => return Put::Other,
    };
    outcome(slice, put)
}

/// What putting a slice came to, with the sentence a slice of the wrong size is refused in.
fn outcome(slice: u32, put: Result<(), HeldError>) -> Put {
    match put {
        Ok(()) => Put::Done,
        Err(HeldError::WrongKind) => Put::Other,
        Err(_) => Put::Failed(
            VolumeError::SizeMismatch {
                slice: slice as usize,
            }
            .to_string(),
        ),
    }
}

/// A volume being filled, and how its slices have gone so far.
struct Filling<'a> {
    held: &'a HeldVolume,
    build: &'a Build,
    /// Whether a slice was stored another way than the volume holds its values.
    other: AtomicBool,
    /// The slices that could not be put, with why.
    failed: Mutex<Vec<(usize, String)>>,
    progress: Progress<'a>,
}

impl Filling<'_> {
    /// Whether to go on: the build is still the newest, and no slice is stored another way.
    fn going(&self, shared: &Shared) -> bool {
        shared.building(self.build) && !self.other.load(Ordering::SeqCst)
    }
}

/// How many slices of a build have been put, reported every [`REPORT_EVERY`].
struct Progress<'a> {
    series: &'a SeriesId,
    of: usize,
    done: AtomicUsize,
    reported: Mutex<usize>,
}

impl<'a> Progress<'a> {
    fn new(series: &'a SeriesId, of: usize) -> Self {
        Self {
            series,
            of,
            done: AtomicUsize::new(0),
            reported: Mutex::new(0),
        }
    }

    /// Count one more slice put, and say so if it is time to and the build is the newest.
    fn one_more(&self, shared: &Shared, build: &Build) {
        let done = self.done.fetch_add(1, Ordering::SeqCst) + 1;
        if !done.is_multiple_of(REPORT_EVERY) || done >= self.of {
            return;
        }
        // Reported with the lock held, so that a later count is never told first.
        let mut reported = lock(&self.reported);
        if done > *reported && shared.building(build) {
            *reported = done;
            shared.report(Action::VolumeProgress {
                series: self.series.clone(),
                done,
                of: self.of,
            });
        }
    }
}

#[cfg(test)]
mod memory_tests;
#[cfg(test)]
mod spill_tests;
#[cfg(test)]
mod tests;
