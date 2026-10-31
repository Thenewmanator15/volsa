//! Slabs put away past a memory budget, and brought back when they are read. Only a whole
//! slab is put away, the one read least lately first; reading one that is away brings it
//! back, and one that cannot be brought back reads as missing.

use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};

use super::{HeldVolume, Values};

/// How many numbers of a slab go to its store, or come back, at once: a mebibyte of 16-bit
/// numbers, so that moving a slab takes little memory besides the slab.
const NUMBERS_AT_ONCE: usize = 1 << 19;

/// Somewhere a held volume's slabs wait while they are not in memory: Voxa's own spill file.
/// A slab goes there, and comes back, a part at a time.
pub trait SlabStore: Send + Sync {
    /// Keep `bytes` as those of slab `slab` from its byte `at` on, in place of any kept there
    /// before.
    ///
    /// # Errors
    ///
    /// When they cannot be kept; the slab then stays in memory.
    fn put_away(&self, slab: usize, at: usize, bytes: &[u8]) -> std::io::Result<()>;

    /// Give back the bytes kept of slab `slab` from its byte `at` on, filling `into`. A part
    /// at `at` 0 is the first of a slab coming back into memory.
    ///
    /// # Errors
    ///
    /// When they cannot be read; the slab then reads as missing.
    fn bring_back(&self, slab: usize, at: usize, into: &mut [u8]) -> std::io::Result<()>;
}

/// Where one slab's numbers are.
pub(super) enum Slab {
    /// Not made: no slice of it has been put.
    Empty,
    /// In memory.
    Here(Values),
    /// Put away in the volume's store.
    Away,
}

impl HeldVolume {
    /// Put slabs away in `store` from now on, when [`HeldVolume::put_away_least_used`] is
    /// asked to. A volume has one store; a second is not taken.
    pub fn keep_away_in(&self, store: Arc<dyn SlabStore>) {
        let _ = self.away.set(store);
    }

    /// Put away the whole slab in memory that was read least lately, and give the bytes of
    /// memory that freed: 0 when there is no store, or no whole slab in memory.
    ///
    /// # Errors
    ///
    /// When the store cannot take the slab, which stays in memory.
    pub fn put_away_least_used(&self) -> std::io::Result<usize> {
        let Some(store) = self.away.get() else {
            return Ok(0);
        };
        let mut whole: Vec<usize> = (0..self.slabs.len())
            .filter(|slab| self.is_whole(*slab) && self.is_here(*slab))
            .collect();
        whole.sort_by_key(|slab| self.read_at.get(*slab).map(|at| at.load(Ordering::SeqCst)));
        for slab in whole {
            let Some(cell) = self.slabs.get(slab) else {
                continue;
            };
            let mut held = cell.write().unwrap_or_else(PoisonError::into_inner);
            let Slab::Here(values) = &*held else {
                continue;
            };
            let size = self.stored.bytes();
            let mut part = Vec::with_capacity(NUMBERS_AT_ONCE * size);
            for first in (0..values.len()).step_by(NUMBERS_AT_ONCE) {
                part.clear();
                values.bytes_of(
                    first..(first + NUMBERS_AT_ONCE).min(values.len()),
                    &mut part,
                );
                store.put_away(slab, first * size, &part)?;
            }
            let freed = values.bytes();
            *held = Slab::Away;
            return Ok(freed);
        }
        Ok(0)
    }

    /// Whether the volume has somewhere to put slabs away.
    pub fn keeps_away(&self) -> bool {
        self.away.get().is_some()
    }

    /// The bytes of a whole slab.
    pub fn slab_bytes(&self) -> usize {
        self.slab_slices(0).len() * self.in_slice() * self.stored.bytes()
    }

    /// Whether slab `slab` is put away.
    pub fn is_away(&self, slab: usize) -> bool {
        self.slabs.get(slab).is_some_and(|cell| {
            matches!(
                *cell.read().unwrap_or_else(PoisonError::into_inner),
                Slab::Away
            )
        })
    }

    fn is_here(&self, slab: usize) -> bool {
        self.slabs.get(slab).is_some_and(|cell| {
            matches!(
                *cell.read().unwrap_or_else(PoisonError::into_inner),
                Slab::Here(_)
            )
        })
    }

    /// Whether every slice of slab `slab` has been put.
    fn is_whole(&self, slab: usize) -> bool {
        self.slab_slices(slab).all(|slice| self.has_slice(slice))
    }

    /// Read slab `slab`'s numbers, bringing them back first if they are away, and count it
    /// as read now. `None` when it has not been made, or cannot be brought back.
    pub(super) fn read_slab<T>(&self, slab: usize, read: impl FnOnce(&Values) -> T) -> Option<T> {
        let cell = self.slabs.get(slab)?;
        if let Some(at) = self.read_at.get(slab) {
            at.store(
                self.clock.fetch_add(1, Ordering::SeqCst) + 1,
                Ordering::SeqCst,
            );
        }
        let mut read = Some(read);
        // Brought back, it may be put away again before it is read: then once more.
        for _ in 0..3 {
            let held = cell.read().unwrap_or_else(PoisonError::into_inner);
            match &*held {
                Slab::Here(values) => return read.take().map(|read| read(values)),
                Slab::Empty => return None,
                Slab::Away => {}
            }
            drop(held);
            self.bring_back(slab)?;
        }
        None
    }

    /// Bring slab `slab` back from the store into memory. `None` when it cannot be.
    fn bring_back(&self, slab: usize) -> Option<()> {
        let cell = self.slabs.get(slab)?;
        let mut held = cell.write().unwrap_or_else(PoisonError::into_inner);
        if !matches!(*held, Slab::Away) {
            return Some(());
        }
        let store = self.away.get()?;
        let (numbers, size) = (
            self.slab_slices(slab).len() * self.in_slice(),
            self.stored.bytes(),
        );
        let mut values = Values::zeroed(self.stored, numbers);
        let mut part = vec![0; NUMBERS_AT_ONCE.min(numbers) * size];
        for first in (0..numbers).step_by(NUMBERS_AT_ONCE) {
            let bytes = part.get_mut(..(numbers - first).min(NUMBERS_AT_ONCE) * size)?;
            store.bring_back(slab, first * size, bytes).ok()?;
            values.set_from_bytes(first, bytes);
        }
        *held = Slab::Here(values);
        Some(())
    }
}
