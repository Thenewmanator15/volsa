//! Slabs put away past a budget come back the same when they are read.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::*;
use crate::grid::Grid;
use crate::space::Affine;

/// Slabs kept in memory, as a file would keep them; it can be told to fail.
#[derive(Default)]
struct Kept {
    slabs: Mutex<HashMap<usize, Vec<u8>>>,
    failing: std::sync::atomic::AtomicBool,
}

impl SlabStore for Kept {
    fn put_away(&self, slab: usize, at: usize, bytes: &[u8]) -> std::io::Result<()> {
        if self.failing.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("full"));
        }
        let mut slabs = self.slabs.lock().expect("held");
        let kept = slabs.entry(slab).or_default();
        kept.resize(kept.len().max(at + bytes.len()), 0);
        kept[at..at + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    fn bring_back(&self, slab: usize, at: usize, into: &mut [u8]) -> std::io::Result<()> {
        if self.failing.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("gone"));
        }
        let slabs = self.slabs.lock().expect("held");
        let kept = slabs
            .get(&slab)
            .and_then(|kept| kept.get(at..at + into.len()))
            .ok_or_else(|| std::io::Error::other("never put"))?;
        into.copy_from_slice(kept);
        Ok(())
    }
}

/// A 2 x 2 x 6 volume of signed numbers, two slices a slab: three slabs of 16 bytes.
fn three_slabs() -> HeldVolume {
    let placed = Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]);
    let held = HeldVolume::new([2, 2, 6], placed, Stored::Signed16, Rescale::default(), 16)
        .expect("a volume");
    for slice in 0..6_i16 {
        let values: Vec<i16> = (0..4).map(|i| -100 + slice * 10 + i).collect();
        held.put_slice(slice.cast_unsigned().into(), SliceIn::Signed(&values))
            .expect("fits");
    }
    held
}

fn every_value(held: &HeldVolume) -> Vec<Option<f32>> {
    (0..6)
        .flat_map(|s| (0..4).map(move |i| held.value(i % 2, i / 2, s)))
        .collect()
}

#[test]
fn a_slab_put_away_comes_back_the_same() {
    let held = three_slabs();
    let before = every_value(&held);
    assert_eq!(
        held.put_away_least_used().expect("nothing to put"),
        0,
        "no store yet"
    );
    held.keep_away_in(Arc::new(Kept::default()));
    assert_eq!(held.bytes_held(), 48);
    assert_eq!(held.put_away_least_used().expect("put"), 16);
    assert_eq!(held.bytes_held(), 32);
    assert_eq!(every_value(&held), before, "read back as they were");
    assert_eq!(held.bytes_held(), 48, "and in memory again");
    assert_eq!(held.slice_values(1), Some(vec![-90.0, -89.0, -88.0, -87.0]));
}

#[test]
fn the_slab_read_least_lately_is_put_away_first() {
    let held = three_slabs();
    held.keep_away_in(Arc::new(Kept::default()));
    let _ = held.value(0, 0, 4);
    let _ = held.value(0, 0, 0);
    held.put_away_least_used().expect("put");
    let away: Vec<bool> = (0..3).map(|slab| held.is_away(slab)).collect();
    assert_eq!(away, [false, true, false], "slab 1 was read least lately");
}

#[test]
fn a_slab_not_yet_whole_is_not_put_away() {
    let placed = Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]);
    let held = HeldVolume::new([2, 2, 4], placed, Stored::Signed16, Rescale::default(), 16)
        .expect("a volume");
    held.put_slice(0, SliceIn::Signed(&[1, 2, 3, 4]))
        .expect("fits");
    held.keep_away_in(Arc::new(Kept::default()));
    assert_eq!(held.put_away_least_used().expect("nothing"), 0);
}

#[test]
fn a_slab_that_cannot_come_back_reads_as_missing() {
    let held = three_slabs();
    let kept = Arc::new(Kept::default());
    held.keep_away_in(Arc::clone(&kept) as Arc<dyn SlabStore>);
    held.put_away_least_used().expect("put");
    let away = (0..3)
        .find(|slab| held.is_away(*slab))
        .expect("one is away");
    kept.failing.store(true, Ordering::SeqCst);
    let slice = u32::try_from(away * 2).expect("small");
    assert_eq!(held.slice_values(slice), None);
    assert_eq!(held.value(0, 0, slice), None);
    kept.failing.store(false, Ordering::SeqCst);
    assert!(
        held.slice_values(slice).is_some(),
        "and comes back when it can"
    );
}

#[test]
fn a_store_that_cannot_take_a_slab_leaves_it_in_memory() {
    let held = three_slabs();
    let kept = Arc::new(Kept::default());
    kept.failing.store(true, Ordering::SeqCst);
    held.keep_away_in(kept);
    assert!(held.put_away_least_used().is_err());
    assert_eq!(held.bytes_held(), 48);
    assert_eq!(held.value(1, 1, 5), Some(-47.0));
}
