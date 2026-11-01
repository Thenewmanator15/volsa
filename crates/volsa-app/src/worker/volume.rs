//! The volume of a series for the 3D scene: asked for, kept, and given way. A series is held
//! once: as its volume while that is the one kept, and as frames in the cache otherwise.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use volsa_core::{Grid, HeldVolume, SeriesId};
use volsa_dicom::{Frame, ImageFile, IndexedSeries};
use volsa_render::{CardRoom, MAX_VOLUME_BYTES, Smoothed, card_mapping};
use volsa_volume::halved_held;

use super::held::Built;
use super::{Shared, Workers, lock, shielded};
use crate::action::Action;
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

/// What the window's graphics device does itself from a volume it holds.
#[derive(Debug, Default)]
pub(super) struct DeviceDoes {
    /// Working out the curve through the values.
    curves: AtomicBool,
}

impl Workers {
    /// Say whether the window's graphics device works out the curve through a volume's
    /// values itself. What it does is left to it from now on, and not done on a worker first.
    /// The workers never hand the device anything: only the window's thread does.
    pub fn leave_to_the_device(&self, curves: bool) {
        let does = &self.shared.device_does;
        does.curves.store(curves, Ordering::SeqCst);
    }

    /// Say what the window's graphics device can hold of a volume, and how. A volume built
    /// from now on that it cannot hold whole is given a halved copy for it.
    pub fn set_card_room(&self, room: CardRoom) {
        *lock(&self.shared.card_room) = room;
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
        // The curve is through what the device holds: the volume, or its halved copy.
        let card = store
            .for_card(&series)
            .map_or_else(|| Arc::clone(&volume), |(copy, _)| copy);
        drop(store);
        // A series the device holds in several textures gets no curve: the curve is one
        // texture, and would take as much room again. Nor one whose 32-bit copy, which the
        // curve is made from here, would pass the memory budget.
        let room = *lock(&self.shared.card_room);
        let voxels = card.dims().iter().map(|&side| side as usize).product();
        if room.textures(card.dims()) != Some(1) || self.shared.copy_passes_budget(voxels) {
            let why = crate::planes::too_long_to_smooth();
            self.shared.report(Action::Post(why));
            return;
        }
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            shielded(|| {
                let room = *lock(&shared.card_room);
                let Some(values) = card.to_volume(0..card.dims()[2]) else {
                    return;
                };
                let limits = (room.longest_side, MAX_VOLUME_BYTES);
                let curve = Smoothed::mapped(&values, limits, Some(card_mapping(&card, &room)));
                drop(values);
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
        self.spawn_build(move || {
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
    pub(super) fn building(&self, build: &Build) -> bool {
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

    /// Build the held volume of a series, keep it in the store, and report how it went.
    /// Nothing is stored or reported for a request that has been overtaken. Slices decoded
    /// for a volume that does not come to be are put in the frame cache: none is decoded for
    /// nothing.
    fn build_volume(&self, listed: &IndexedSeries, build: &Build) {
        let series = listed.info.id.clone();
        let (left, failed) = match self.build_held(listed, build) {
            Built::Whole(held) => match self.keep(&series, held, build) {
                None => return,
                Some(held) => (Some(held), None),
            },
            Built::Failed(held, reason) => (held, Some(reason)),
            Built::Overtaken(held) => (Some(held), None),
        };
        if let Some(held) = left {
            self.keep_slices(&series, &held, build.opened);
        }
        if let Some(reason) = failed.filter(|_| self.ended(build)) {
            self.report(Action::VolumeFailed { series, reason });
        }
    }

    /// Keep a volume in the store, with a halved copy for the graphics device if it cannot
    /// hold the volume whole, and say it is ready. The volume is given back, with nothing
    /// kept or said, if the request has been overtaken.
    fn keep(&self, series: &SeriesId, held: HeldVolume, build: &Build) -> Option<HeldVolume> {
        let corners = held.corners();
        // Made here, on a worker, so that the window has only to hand it over.
        let room = *lock(&self.card_room);
        let for_card = for_the_card(&held, &room);
        // Checked with the store locked, so a newer volume is never overwritten.
        let mut store = lock(&self.volumes);
        if !self.ended(build) {
            return Some(held);
        }
        let before = store.put(series.clone(), Arc::new(held));
        if let Some(copy) = for_card {
            store.put_for_card(series, copy);
        }
        // The volume is where the series' slices are read from now: no second copy is kept.
        lock(&self.cache).forget(series);
        drop(store);
        self.share_the_budget();
        // Taken out first: the store is locked again to count what is held.
        let kept = lock(&self.volumes).kept();
        if let Some(kept) = kept {
            self.keep_within(&kept, false);
        }
        let series = series.clone();
        self.report(Action::VolumeReady {
            series: series.clone(),
            corners,
        });
        if let Some((was, old)) = before.filter(|(was, _)| *was != series) {
            self.keep_slices(&was, &old, build.opened);
        }
        None
    }

    /// Put the slices of a volume that has given way to another, or that did not come to be,
    /// into the frame cache, as many as it has room for, so that going back to its series
    /// decodes nothing again. Slices it does not hold are passed over.
    fn keep_slices(&self, series: &SeriesId, volume: &HeldVolume, opened: u64) {
        let [cols, rows, slices] = volume.dims();
        let middle = slices as usize / 2;
        for slice in 0..slices {
            let Some(values) = volume.slice_values(slice) else {
                continue;
            };
            let frame = Frame {
                rows,
                cols,
                values,
                colour: None,
            };
            let frame = Arc::new(frame);
            if !self.hold(series, slice as usize, frame, (middle, opened), false) {
                return;
            }
        }
    }

    /// Put a decoded slice of a series in the frame cache, for the viewport to show, unless
    /// the series' volume is the one kept: its slices are then read from that, and are not
    /// held a second time. False, with nothing done, when another export has been opened
    /// since the one numbered `opened`: the cache is that export's now. `announced` says
    /// whether its windows have been said; if not, they are said when it is shown.
    pub(super) fn hold(
        &self,
        series: &SeriesId,
        slice: usize,
        frame: Arc<Frame>,
        (keep_near, opened): (usize, u64),
        announced: bool,
    ) -> bool {
        // Wherever both are locked, the store is locked first.
        let volumes = lock(&self.volumes);
        let mut cache = lock(&self.cache);
        if self.opened.load(Ordering::SeqCst) != opened {
            return false;
        }
        if !volumes.holds(series) {
            if announced {
                cache.insert(series, slice, frame, keep_near);
            } else {
                cache.insert_unannounced(series, slice, frame, keep_near);
            }
        }
        true
    }

    /// Read and parse one file of the source that is open.
    pub(super) fn open(&self, entry: &str) -> Result<ImageFile, String> {
        let Some(bytes) = self.read_entry(entry) else {
            return Err("No scan is open.".to_owned());
        };
        bytes
            .and_then(|bytes| ImageFile::open(&bytes))
            .map_err(|why| why.to_string())
    }
}

/// A copy of a volume halved as often as the device needs it to be to hold it, and how
/// often; `None` when it holds the volume whole, or the volume cannot be halved.
fn for_the_card(held: &HeldVolume, room: &CardRoom) -> Option<(Arc<HeldVolume>, u32)> {
    let halvings = room.halvings(held.dims());
    if halvings == 0 {
        return None;
    }
    let mut copy = halved_held(held)?;
    for _ in 1..halvings {
        copy = halved_held(&copy)?;
    }
    Some((Arc::new(copy), halvings))
}
