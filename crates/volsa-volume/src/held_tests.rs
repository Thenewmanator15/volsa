//! A held volume reads, along lines and over regions, as a volume of the same values does.

use volsa_core::{Patient, Point};

use crate::measure::ellipse_stats;
use crate::plane::{PlaneKind, plane_of};
use crate::ray::{Ray, first_hit};
use crate::sample::sample;
use crate::testing::{ball, held_from};

#[test]
fn a_held_volume_samples_as_the_volume_does() {
    let volume = ball();
    let held = held_from(&volume);
    for at in [
        [10.0, 10.0, 10.0],
        [7.3, 9.1, 10.6],
        [-0.4, 0.0, 20.4],
        [30.0, 0.0, 0.0],
    ] {
        let point: Point<Patient> = Point::new(at[0], at[1], at[2]);
        assert_eq!(sample(&held, point), sample(&volume, point), "at {at:?}");
    }
}

#[test]
fn a_held_volume_is_hit_where_the_volume_is() {
    let volume = ball();
    let held = held_from(&volume);
    let ray = Ray {
        origin: Point::new(-5.0, 10.0, 10.0),
        direction: [1.0, 0.0, 0.0],
    };
    let hit = first_hit(&held, &ray, 20.0).expect("hits the ball");
    assert_eq!(Some(hit), first_hit(&volume, &ray, 20.0));
}

#[test]
fn a_region_of_a_held_volume_measures_as_the_volume_does() {
    let volume = ball();
    let held = held_from(&volume);
    let middle = Point::new(10.0, 10.0, 10.0);
    let plane = plane_of(
        &volume.voxel_to_patient(),
        volume.dims(),
        PlaneKind::Axial,
        middle,
        true,
    )
    .expect("a plane");
    let found = ellipse_stats(&held, &plane, [0.0, 0.0], [4.0, 3.0]).expect("inside");
    assert_eq!(
        Some(found),
        ellipse_stats(&volume, &plane, [0.0, 0.0], [4.0, 3.0])
    );
}

#[test]
fn a_held_volume_halved_holds_the_rounded_means_where_the_volume_halved_lies() {
    use volsa_core::{Affine, Grid, HeldVolume, Rescale, SliceIn, Stored};

    use crate::reduce::{halved, halved_held};

    let ct = Rescale {
        slope: 1.0,
        intercept: -1024.0,
    };
    let placed = Affine::from_columns([0.5, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 2.0], [-3.0; 3]);
    let held = HeldVolume::new([5, 2, 3], placed, Stored::Unsigned16, ct, 1 << 20).expect("ok");
    for slice in 0..3_u16 {
        let numbers: Vec<u16> = (0..10).map(|i| 1000 + 7 * i + 31 * slice).collect();
        held.put_slice(u32::from(slice), SliceIn::Unsigned(&numbers))
            .expect("fits");
    }
    let half = halved_held(&held).expect("halved");
    assert_eq!(half.dims(), [3, 1, 2]);
    assert_eq!((half.stored(), half.rescale()), (Stored::Unsigned16, ct));
    let plain = halved(&held.to_volume(0..3).expect("whole"));
    assert_eq!(half.corners(), plain.corners());
    for (col, slice) in [(0, 0), (1, 0), (2, 0), (0, 1), (2, 1)] {
        let (got, want) = (half.value(col, 0, slice), plain.get(col, 0, slice));
        let off = (got.expect("a value") - want.expect("a value")).abs();
        assert!(off <= 0.5, "({col}, {slice}): {got:?} against {want:?}");
    }
}
