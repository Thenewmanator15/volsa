//! A susceptibility map worked out on a worker from the phase volume kept for a series, and
//! kept in that volume's place, so that the slices, the planes and the 3D view all show it. It
//! is said to be ready as a volume is. Asking again, or giving it up, overtakes a map being
//! worked out; another export opened, or another volume kept, does too.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use volsa_core::{Grid as _, HeldVolume, Rescale, SeriesId, SliceIn, Stored};
use volsa_segment::{PhaseFacts, susceptibility};

use super::volume::for_the_card;
use super::{Shared, Workers, lock, shielded};
use crate::action::{Action, Effect};

/// What is said when the phase's volume is no longer there.
const NO_VOLUME: &str = "The 3D view of this phase is no longer there to work out a map from.";
/// What is said when there is not the memory to hold the map.
const NO_MEMORY: &str = "There is not the memory to hold a susceptibility map of this scan.";
/// What is said when the work stops part-way inside Volsa.
const STOPPED: &str = "The susceptibility map stopped part-way: something went wrong inside Volsa.";

impl Workers {
    /// Do what an effect about a susceptibility map asks.
    pub(super) fn map(&self, effect: Effect) {
        match effect {
            Effect::MakeSusceptibility { series, facts } => self.make_map(series, facts),
            Effect::DropSusceptibility => self.drop_map(),
            _ => {}
        }
    }

    /// Work out the susceptibility map of the phase volume kept for `series` on a worker, keep
    /// it in that volume's place and say it is ready, or say why it could not be made. Nothing
    /// is kept or said once it has been overtaken.
    fn make_map(&self, series: SeriesId, facts: PhaseFacts) {
        let number = self.shared.maps.fetch_add(1, Ordering::SeqCst) + 1;
        let opened = self.shared.opened.load(Ordering::SeqCst);
        let phase = lock(&self.shared.volumes).own(&series);
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            let current = || {
                shared.maps.load(Ordering::SeqCst) == number
                    && shared.opened.load(Ordering::SeqCst) == opened
            };
            let mut outcome = Err(STOPPED.to_owned());
            shielded(|| {
                outcome = match phase.as_deref() {
                    Some(phase) => shared.map_of(phase, facts, &|| !current()),
                    None => Err(NO_VOLUME.to_owned()),
                };
            });
            // The phase is let go before the map takes its place.
            drop(phase);
            match outcome {
                Ok(map) => {
                    shared.keep_map(&series, map, &current);
                }
                Err(reason) if current() => {
                    shared.report(Action::VolumeFailed { series, reason });
                }
                Err(_) => {}
            }
        });
    }

    /// Give up the map kept, if one is, and any being worked out.
    fn drop_map(&self) {
        self.shared.maps.fetch_add(1, Ordering::SeqCst);
        if lock(&self.shared.volumes).drop_map() {
            self.shared.share_the_budget();
        }
    }
}

impl Shared {
    /// The susceptibility map of `phase`, held as 32-bit values on the phase's own grid, put
    /// on disk slab by slab as a volume being built is when it passes the budget. `stop` is
    /// asked as it is worked out.
    fn map_of(
        &self,
        phase: &HeldVolume,
        facts: PhaseFacts,
        stop: &(dyn Fn() -> bool + Sync),
    ) -> Result<HeldVolume, String> {
        let dims = phase.dims();
        // Worked out from 32-bit values of the whole scan, as labelling is: made here, for
        // this alone.
        let values = phase.to_volume(0..dims[2]).ok_or(NO_MEMORY)?;
        let map = susceptibility(&values, facts, stop).map_err(|why| why.to_string())?;
        drop(values);
        let (stored, rescale) = (Stored::Float32, Rescale::default());
        let placed = map.voxel_to_patient();
        let held = HeldVolume::new(dims, placed, stored, rescale, self.slab_bytes());
        let held = held.ok_or(NO_MEMORY)?;
        let in_slice = (dims[0] as usize).saturating_mul(dims[1] as usize).max(1);
        for slab in 0..held.slab_count() {
            for slice in held.slab_slices(slab) {
                let from = (slice as usize).saturating_mul(in_slice);
                let values = map.values().get(from..from.saturating_add(in_slice));
                held.put_slice(slice, SliceIn::Float(values.ok_or(NO_MEMORY)?))
                    .map_err(|_| NO_MEMORY.to_owned())?;
            }
            self.keep_within(&held, true);
        }
        Ok(held)
    }

    /// Keep a map of `series` in place of its volume, with a halved copy for the graphics
    /// device if it cannot hold the map whole, and say it is ready as a volume is. Nothing is
    /// kept or said when the map has been overtaken (`current` says not), or the series' own
    /// volume is no longer the one kept. Whether it was kept.
    fn keep_map(&self, series: &SeriesId, map: HeldVolume, current: &dyn Fn() -> bool) -> bool {
        let corners = map.corners();
        // Made here, on a worker, so that the window has only to hand it over.
        let room = *lock(&self.card_room);
        let for_card = for_the_card(&map, &room);
        // Checked with the store locked, so that a newer volume is never overwritten.
        let mut store = lock(&self.volumes);
        if !current() || store.own(series).is_none() {
            return false;
        }
        store.put_map(series.clone(), Arc::new(map));
        if let Some(copy) = for_card {
            store.put_for_card(series, copy);
        }
        // The map is where the series' slices are read from now: none of the phase is kept.
        lock(&self.cache).forget(series);
        drop(store);
        self.share_the_budget();
        let kept = lock(&self.volumes).kept();
        if let Some(kept) = kept {
            self.keep_within(&kept, false);
        }
        self.report(Action::VolumeReady {
            series: series.clone(),
            corners,
        });
        true
    }
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "the made-up phase is small, and its angles fit an f32"
)]
mod tests;
