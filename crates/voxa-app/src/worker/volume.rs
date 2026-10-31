//! Building the volume of a series for the 3D scene: every slice decoded, then stacked. A
//! series is held once: as its volume while that is the one kept, and as frames in the cache
//! otherwise.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use egui_wgpu::wgpu;
use rayon::prelude::*;
use voxa_core::{SeriesId, Volume};
use voxa_dicom::{Frame, ImageFile, IndexedSeries};
use voxa_render::{MAX_VOLUME_BYTES, Prepared, Smoothed};
use voxa_volume::{PRESETS, assemble};

use super::opened::batches;
use super::{Batch, Shared, Workers, lock, shielded};
use crate::action::Action;

/// Progress is reported each time this many more slices have been decoded.
const REPORT_EVERY: usize = 8;
/// What is said when the reader gives up part-way through a series.
pub(super) const READER_STOPPED: &str =
    "This series could not be read for a 3D view: the reader stopped on one of its files";

/// One request for the volume of a series.
pub(super) struct Build {
    /// Which source was open when the request was made.
    opened: u64,
    /// Which request for a volume it is. Only the newest is carried through.
    number: u64,
}

/// A slice's place in its series, and the slice decoded or why it could not be.
type Decoded = (usize, Result<Arc<Frame>, String>);

/// What the window's graphics device does itself from a volume it holds.
#[derive(Debug, Default)]
pub(super) struct DeviceDoes {
    /// Working out the largest value in each block of voxels.
    blocks: AtomicBool,
    /// Working out the curve through the values.
    curves: AtomicBool,
    /// The device itself and its queue, once the window has given them: a volume made
    /// ready is then handed to the device by the worker that made it.
    device: Mutex<Option<(wgpu::Device, wgpu::Queue)>>,
}

impl Workers {
    /// Give the workers the window's graphics device. From now on the worker that makes a
    /// volume ready also hands its values to the device, so that the window, when it
    /// shows the volume, has no bytes left to hand over and does not pause to.
    pub fn give_device(&self, device: wgpu::Device, queue: wgpu::Queue) {
        *lock(&self.shared.device_does.device) = Some((device, queue));
    }

    /// Say what the window's graphics device does itself: the largest value in each block
    /// of a volume, and the curve through its values. What it does is left to it from now
    /// on, and not done on a worker first.
    pub fn leave_to_the_device(&self, blocks: bool, curves: bool) {
        let does = &self.shared.device_does;
        does.blocks.store(blocks, Ordering::SeqCst);
        does.curves.store(curves, Ordering::SeqCst);
    }

    /// Say how long a side, in voxels, the window's graphics device allows a volume. Volumes
    /// built from now on are made ready to fit it.
    pub fn set_volume_side(&self, longest: u32) {
        self.shared.volume_side.store(longest, Ordering::SeqCst);
    }

    /// Build the volume of a series for the 3D scene, in place of any being built.
    /// The curve through a series' volume is wanted. Where the graphics device makes it
    /// from the volume it holds, nothing is done here; otherwise a worker works it out.
    pub(super) fn build_smooth(&self, series: SeriesId) {
        if !self.shared.device_does.curves.load(Ordering::SeqCst) {
            self.work_out_curve(series);
        }
    }

    /// Work out, on a worker, the curve through the volume kept for a series, keep it with
    /// the volume and say it is ready, whatever the graphics device does itself: this is
    /// also what is asked when the device turns out to have no room to. Nothing is done
    /// when that series' volume is not the one kept, and nothing is kept or said if the
    /// volume has given way meanwhile, or the curve is no longer wanted.
    pub fn work_out_curve(&self, series: SeriesId) {
        let mut store = lock(&self.shared.volumes);
        store.want_smoothed(true);
        let Some(volume) = store.get(&series) else {
            return;
        };
        drop(store);
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            shielded(|| {
                let side = shared.volume_side.load(Ordering::SeqCst);
                let curve = Smoothed::of(&volume, side, MAX_VOLUME_BYTES);
                if lock(&shared.volumes).put_smoothed(&series, &volume, curve) {
                    shared.report(Action::SmoothReady { series });
                }
            });
        });
    }

    pub(super) fn build_volume(&self, listed: &IndexedSeries) {
        // Said at once, before the work starts, so that frames asked for next are left to it.
        let build = self.shared.wants(&listed.info.id);
        let (shared, listed) = (Arc::clone(&self.shared), listed.clone());
        self.spawn(move || {
            let finished = shielded(|| shared.build_volume(&listed, &build));
            if !finished && shared.ended(&build) {
                // Without this the scene would say it was building for ever.
                shared.report(Action::VolumeFailed {
                    series: listed.info.id.clone(),
                    reason: READER_STOPPED.to_owned(),
                });
            }
        });
    }
}

impl Shared {
    /// A new request for the volume of a series. Any volume being built for an earlier one
    /// is abandoned, and the slices of this series are from now decoded for its volume.
    fn wants(&self, series: &SeriesId) -> Build {
        // Both with the one lock held, so that an older request ending cannot undo this.
        let mut wanted = lock(&self.wanted_volume);
        *wanted = Some(series.clone());
        Build {
            opened: self.opened.load(Ordering::SeqCst),
            number: self.builds.fetch_add(1, Ordering::SeqCst) + 1,
        }
    }

    /// Whether a request for a volume is still the newest, in the source still open.
    fn building(&self, build: &Build) -> bool {
        self.opened.load(Ordering::SeqCst) == build.opened
            && self.builds.load(Ordering::SeqCst) == build.number
    }

    /// Note that a request for a volume is over, so that slices of its series are decoded
    /// for the viewport again. False, with nothing noted, if the request had been overtaken.
    fn ended(&self, build: &Build) -> bool {
        let mut wanted = lock(&self.wanted_volume);
        let newest = self.building(build);
        if newest {
            *wanted = None;
        }
        newest
    }

    /// Decode every slice of a series, taking those already in the frame cache from there,
    /// stack them into a volume, keep it in the store, and report how it went. Nothing is
    /// stored or reported for a request that has been overtaken. Slices decoded for a volume
    /// that does not come to be are put in the frame cache: none is decoded for nothing.
    fn build_volume(&self, listed: &IndexedSeries, build: &Build) {
        let series = listed.info.id.clone();
        let decoded = self.decode_all(listed, build);
        let stacked = self.building(build).then(|| stacked(listed, &decoded));
        let failed = match stacked {
            Some(Ok(volume)) => {
                if self.keep(&series, volume, build) {
                    return;
                }
                None
            }
            Some(Err(reason)) => Some(reason),
            None => None,
        };
        self.keep_frames(&series, decoded, build);
        if let Some(reason) = failed.filter(|_| self.ended(build)) {
            self.report(Action::VolumeFailed { series, reason });
        }
    }

    /// Every slice of a series in the series' order, each decoded or why it could not be,
    /// with progress reported on the way. Fewer than all once the request is overtaken.
    fn decode_all(&self, listed: &IndexedSeries, build: &Build) -> Vec<Decoded> {
        let series = &listed.info.id;
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
        let from_each = |file: &Batch| self.slices_of(file, series, build, &one_more);
        let mut decoded: Vec<Decoded> = files.par_iter().flat_map_iter(from_each).collect();
        decoded.sort_by_key(|(slice, _)| *slice);
        decoded
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
                || {
                    let image = opened.get_or_insert_with(|| self.open(&file.entry));
                    let fresh = decoded_from(image, *frame)?;
                    self.announce(series, *slice, &fresh, build);
                    Ok(fresh)
                },
                Ok,
            );
            slices.push((*slice, decoded));
            one_more();
        }
        slices
    }

    /// Say that a slice has been decoded, with the window each preset gives for it, as is
    /// said of a slice decoded for the viewport: the viewport shows it, out of the volume
    /// once that is built, and needs its windows.
    fn announce(&self, series: &SeriesId, slice: usize, decoded: &Frame, build: &Build) {
        if self.building(build) {
            let windows = PRESETS.map(|preset| preset.window(&decoded.values));
            let series = series.clone();
            self.report(Action::FrameReady {
                series,
                slice,
                windows,
            });
        }
    }

    /// Keep a volume in the store, with what makes it ready for the graphics device, and
    /// say it is ready. False, with nothing kept or said, if the request has been overtaken.
    fn keep(&self, series: &SeriesId, volume: Volume, build: &Build) -> bool {
        let corners = volume.corners();
        // Made ready for the graphics device here, on a worker, so that the window has only
        // to hand it over.
        let side = self.volume_side.load(Ordering::SeqCst);
        let mut ready = if self.device_does.blocks.load(Ordering::SeqCst) {
            Prepared::without_blocks(&volume, side, MAX_VOLUME_BYTES)
        } else {
            Prepared::of(&volume, side, MAX_VOLUME_BYTES)
        };
        // Handed to the device from here too. With no room there it stays as it is, and
        // the window deals with that as it always has.
        let device = lock(&self.device_does.device).clone();
        if let Some((device, queue)) = device {
            ready.hand_over(&device, &queue);
        }
        // Checked with the store locked, so a newer volume is never overwritten.
        let mut store = lock(&self.volumes);
        if !self.ended(build) {
            return false;
        }
        let before = store.put(series.clone(), Arc::new(volume));
        store.put_prepared(series, ready);
        // The volume is where the series' slices are read from now: no second copy is kept.
        lock(&self.cache).forget(series);
        drop(store);
        let series = series.clone();
        self.report(Action::VolumeReady {
            series: series.clone(),
            corners,
        });
        if let Some((was, old)) = before.filter(|(was, _)| *was != series) {
            self.keep_slices(&was, &old, build.opened);
        }
        true
    }

    /// Put the slices of a volume that has given way to another into the frame cache, as
    /// many as it has room for, so that going back to its series decodes nothing again.
    fn keep_slices(&self, series: &SeriesId, volume: &Volume, opened: u64) {
        let [cols, rows, slices] = volume.dims();
        let middle = slices as usize / 2;
        for slice in 0..slices {
            let Some(values) = volume.slice_values(slice) else {
                return;
            };
            let frame = Frame {
                rows,
                cols,
                values: values.to_vec(),
                colour: None,
            };
            if !self.hold(series, slice as usize, Arc::new(frame), middle, opened) {
                return;
            }
        }
    }

    /// Put what was decoded for a volume that did not come to be into the frame cache, as
    /// much as it has room for: the viewport can show it, and it is not decoded again.
    fn keep_frames(&self, series: &SeriesId, decoded: Vec<Decoded>, build: &Build) {
        let middle = decoded.len() / 2;
        for (slice, frame) in decoded {
            let Ok(frame) = frame else {
                continue;
            };
            if !self.hold(series, slice, frame, middle, build.opened) {
                return;
            }
        }
    }

    /// Put a decoded slice of a series in the frame cache, for the viewport to show, unless
    /// the series' volume is the one kept: its slices are then read from that, and are not
    /// held a second time. False, with nothing done, when another export has been opened
    /// since the one numbered `opened`: the cache is that export's now.
    pub(super) fn hold(
        &self,
        series: &SeriesId,
        slice: usize,
        frame: Arc<Frame>,
        keep_near: usize,
        opened: u64,
    ) -> bool {
        // Wherever both are locked, the store is locked first.
        let volumes = lock(&self.volumes);
        let mut cache = lock(&self.cache);
        if self.opened.load(Ordering::SeqCst) != opened {
            return false;
        }
        if !volumes.holds(series) {
            cache.insert(series, slice, frame, keep_near);
        }
        true
    }

    /// Read and parse one file of the source that is open.
    fn open(&self, entry: &str) -> Result<ImageFile, String> {
        let Some(bytes) = self.read_entry(entry) else {
            return Err("No scan is open.".to_owned());
        };
        bytes
            .and_then(|bytes| ImageFile::open(&bytes))
            .map_err(|why| why.to_string())
    }
}

/// One slice decoded from a file that was read, or why either could not be done.
fn decoded_from(
    image: &Result<ImageFile, String>,
    frame: Option<u32>,
) -> Result<Arc<Frame>, String> {
    match image {
        Ok(image) => image
            .decode(frame)
            .map(Arc::new)
            .map_err(|why| why.to_string()),
        Err(why) => Err(why.clone()),
    }
}

/// The decoded slices of a series, given in the series' order, stacked into a volume; or why
/// they cannot be, in plain words, naming the first slice that would not decode.
fn stacked(listed: &IndexedSeries, decoded: &[Decoded]) -> Result<Volume, String> {
    let mut values = Vec::with_capacity(decoded.len());
    for (slice, frame) in decoded {
        let number = slice + 1;
        let frame = frame.as_ref().map_err(|why| {
            format!("Slice {number} could not be decoded, so a 3D view cannot be built: {why}")
        })?;
        values.push(frame.values.as_slice());
    }
    assemble(&listed.info, &values).map_err(|why| why.to_string())
}
