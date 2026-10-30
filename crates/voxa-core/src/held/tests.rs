use std::sync::Arc;

use super::*;
use crate::space::{Affine, Point};

/// Voxels half a millimetre across in slices 2 mm apart, away from the origin.
fn placed() -> Affine<Voxel, Patient> {
    Affine::from_columns(
        [0.5, 0.0, 0.0],
        [0.0, 0.5, 0.0],
        [0.0, 0.0, 2.0],
        [-100.0, -120.0, 10.0],
    )
}

fn ct() -> Rescale {
    Rescale {
        slope: 1.0,
        intercept: -1024.0,
    }
}

/// A 2 x 2 x 3 volume of unsigned values counting up from 1022, with a CT's rescale.
fn counted() -> HeldVolume {
    let held =
        HeldVolume::new([2, 2, 3], placed(), Stored::Unsigned16, ct(), 1 << 20).expect("a volume");
    for slice in 0..3_u16 {
        let values: Vec<u16> = (0..4).map(|i| 1022 + slice * 4 + i).collect();
        held.put_slice(u32::from(slice), SliceIn::Unsigned(&values))
            .expect("fits");
    }
    held
}

#[test]
fn a_value_is_its_stored_number_rescaled() {
    let held = counted();
    assert_eq!(held.value(0, 0, 0), Some(-2.0));
    assert_eq!(held.value(0, 1, 0), Some(0.0), "1024 stored is 0 HU");
    assert_eq!(held.value(1, 1, 2), Some(9.0));
    assert_eq!(held.value(2, 0, 0), None, "outside");
    assert_eq!(held.value(0, 0, 3), None, "outside");
}

#[test]
fn signed_values_read_below_zero() {
    let held = HeldVolume::new([2, 1, 2], placed(), Stored::Signed16, ct(), 1 << 20).expect("ok");
    held.put_slice(0, SliceIn::Signed(&[-1, 0])).expect("fits");
    held.put_slice(1, SliceIn::Signed(&[-32768, 32767]))
        .expect("fits");
    assert_eq!(held.value(0, 0, 0), Some(-1025.0));
    assert_eq!(held.value(0, 0, 1), Some(-33792.0));
    assert_eq!(held.value(1, 0, 1), Some(31743.0));
}

#[test]
fn float_values_read_as_given() {
    let held = HeldVolume::new(
        [2, 1, 2],
        placed(),
        Stored::Float32,
        Rescale::default(),
        1 << 20,
    )
    .expect("ok");
    held.put_slice(0, SliceIn::Float(&[0.25, f32::NAN]))
        .expect("fits");
    held.put_slice(1, SliceIn::Float(&[-7.5, 1e9]))
        .expect("fits");
    assert_eq!(held.value(0, 0, 0), Some(0.25));
    assert!(held.value(1, 0, 0).is_some_and(f32::is_nan));
    assert_eq!(held.value(1, 0, 1), Some(1e9));
}

#[test]
fn slices_land_in_their_slabs() {
    // Slices of 4 x 4 values of 2 bytes: three fit a slab of 96 bytes.
    let held =
        HeldVolume::new([4, 4, 10], placed(), Stored::Unsigned16, ct(), 96).expect("a volume");
    assert_eq!(held.slab_count(), 4);
    let ranges: Vec<_> = (0..4).map(|slab| held.slab_slices(slab)).collect();
    assert_eq!(ranges, [0..3, 3..6, 6..9, 9..10]);
    held.put_slice(7, SliceIn::Unsigned(&[1030; 16]))
        .expect("fits");
    let seen = held.with_slab(2, |raw| match raw {
        SlabRaw::Unsigned(values) => values.get(16..32).map(<[u16]>::to_vec),
        _ => None,
    });
    assert_eq!(
        seen,
        Some(Some(vec![1030; 16])),
        "slice 7 is the second of slab 2"
    );
    assert_eq!(held.with_slab(0, |_| ()), None, "nothing put in slab 0 yet");
    assert_eq!(held.bytes_held(), 96, "one slab made");
}

#[test]
fn a_slice_of_the_wrong_size_or_kind_is_refused() {
    let held = counted();
    assert_eq!(
        held.put_slice(0, SliceIn::Unsigned(&[1; 3])),
        Err(HeldError::WrongSize)
    );
    assert_eq!(
        held.put_slice(0, SliceIn::Signed(&[1; 4])),
        Err(HeldError::WrongKind)
    );
    assert_eq!(
        held.put_slice(3, SliceIn::Unsigned(&[1; 4])),
        Err(HeldError::NoSuchSlice)
    );
}

#[test]
fn a_volume_is_complete_once_every_slice_is_put() {
    let held =
        Arc::new(HeldVolume::new([4, 4, 40], placed(), Stored::Unsigned16, ct(), 64).expect("ok"));
    assert!(!held.is_complete());
    let threads: Vec<_> = (0..8_u32)
        .map(|thread| {
            let held = Arc::clone(&held);
            std::thread::spawn(move || {
                for slice in (thread..40).step_by(8) {
                    let value = u16::try_from(slice).expect("small");
                    held.put_slice(slice, SliceIn::Unsigned(&[value; 16]))
                        .expect("fits");
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("no thread panicked");
    }
    assert!(held.is_complete());
    assert!((0..40).all(|slice| held.has_slice(slice)));
    assert_eq!(held.value(3, 3, 39), Some(39.0 - 1024.0));
}

#[test]
fn the_values_of_a_slice_are_its_rescaled_numbers() {
    let held = counted();
    assert_eq!(held.slice_values(1), Some(vec![2.0, 3.0, 4.0, 5.0]));
    let fresh =
        HeldVolume::new([2, 2, 3], placed(), Stored::Unsigned16, ct(), 1 << 20).expect("ok");
    assert_eq!(fresh.slice_values(0), None, "a slice not put has no values");
}

#[test]
fn the_values_of_a_range_of_slices_make_a_volume_placed_at_its_first_slice() {
    let held = counted();
    let part = held.to_volume(1..3).expect("a volume");
    assert_eq!(part.dims(), [2, 2, 2]);
    assert_eq!(part.get(0, 0, 0), Some(2.0));
    let first = part.voxel_to_patient().apply(Point::new(0.0, 0.0, 0.0));
    assert_eq!((first.x, first.y, first.z), (-100.0, -120.0, 12.0));
    assert!(held.to_volume(2..4).is_none(), "past the last slice");
    assert!(held.to_volume(1..1).is_none(), "no slices");
}

#[test]
fn corners_and_spacing_are_those_of_a_volume_with_the_same_placement() {
    let held = counted();
    let same = Volume::new([2, 2, 3], vec![0.0; 12], placed()).expect("adds up");
    assert_eq!(Grid::corners(&held), same.corners());
    assert_eq!(Grid::spacing_mm(&held), same.spacing_mm());
    assert_eq!(Grid::dims(&held), [2, 2, 3]);
}

#[test]
fn a_volume_with_no_voxels_or_a_flat_placement_is_refused() {
    let flat = Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0; 3], [0.0; 3]);
    let make = |dims, placement| HeldVolume::new(dims, placement, Stored::Unsigned16, ct(), 64);
    assert!(make([0, 2, 2], placed()).is_none());
    assert!(make([2, 2, 2], flat).is_none());
}
