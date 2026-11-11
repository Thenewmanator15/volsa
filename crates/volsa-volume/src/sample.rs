//! Reading a volume between its voxels.

use voxa_core::{Patient, Point, Volume};

/// The value of a volume at a point in the patient, blended from the eight voxels around it.
/// `None` outside the volume: more than half a voxel beyond the centres of its edge voxels.
/// Within that last half voxel the edge value is used.
pub fn sample(volume: &Volume, at: Point<Patient>) -> Option<f32> {
    let voxel = volume.patient_to_voxel().apply(at);
    let [cols, rows, slices] = volume.dims();
    let (col, next_col, across) = between(voxel.x, cols)?;
    let (row, next_row, down) = between(voxel.y, rows)?;
    let (slice, next_slice, through) = between(voxel.z, slices)?;
    let along_columns = |in_row, in_slice| {
        let from = volume.get(col, in_row, in_slice)?;
        let to = volume.get(next_col, in_row, in_slice)?;
        Some(mix(from, to, across))
    };
    let near = mix(
        along_columns(row, slice)?,
        along_columns(next_row, slice)?,
        down,
    );
    let far = mix(
        along_columns(row, next_slice)?,
        along_columns(next_row, next_slice)?,
        down,
    );
    Some(mix(near, far, through))
}

/// The two voxels a coordinate lies between along one axis of `count` voxels, and how far it
/// is from the first towards the second, from 0 to 1. `None` more than half a voxel outside.
fn between(coord: f64, count: u32) -> Option<(u32, u32, f32)> {
    let last = f64::from(count.checked_sub(1)?);
    // A coordinate that is not a number is in no range, so it is refused here too.
    if !(-0.5..=last + 0.5).contains(&coord) {
        return None;
    }
    let within = coord.clamp(0.0, last);
    let low = whole(within);
    Some((
        low,
        (low + 1).min(count - 1),
        narrowed(within - within.floor()),
    ))
}

fn mix(from: f32, to: f32, part: f32) -> f32 {
    from + (to - from) * part
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and the last voxel first"
)]
fn whole(coord: f64) -> u32 {
    coord.floor() as u32
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a fraction between 0 and 1"
)]
fn narrowed(part: f64) -> f32 {
    part as f32
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use voxa_core::Affine;

    use super::*;
    use crate::testing::ball;

    #[test]
    fn a_point_at_a_voxel_centre_has_that_voxels_value() {
        let volume = ball();
        assert_eq!(sample(&volume, Point::new(10.0, 10.0, 10.0)), Some(100.0));
        assert_eq!(sample(&volume, Point::new(0.0, 0.0, 0.0)), Some(0.0));
    }

    #[test]
    fn a_point_between_voxels_is_blended_from_them() {
        let volume = ball();
        assert_eq!(sample(&volume, Point::new(6.5, 10.0, 10.0)), Some(50.0));
        assert_eq!(sample(&volume, Point::new(6.25, 10.0, 10.0)), Some(25.0));
        assert_eq!(
            sample(&volume, Point::new(10.0, 13.5, 10.0)),
            Some(50.0),
            "along rows"
        );
        assert_eq!(
            sample(&volume, Point::new(10.0, 10.0, 13.75)),
            Some(25.0),
            "along slices"
        );
    }

    #[test]
    fn a_point_outside_the_volume_has_no_value() {
        let volume = ball();
        assert_eq!(sample(&volume, Point::new(-0.6, 10.0, 10.0)), None);
        assert_eq!(sample(&volume, Point::new(10.0, 20.6, 10.0)), None);
        assert_eq!(sample(&volume, Point::new(10.0, 10.0, f64::NAN)), None);
    }

    #[test]
    fn the_last_half_voxel_has_the_value_of_the_edge_voxel() {
        let edge = Volume::new([2, 1, 1], vec![4.0, 8.0], Affine::identity()).expect("adds up");
        assert_eq!(sample(&edge, Point::new(-0.4, 0.0, 0.0)), Some(4.0));
        assert_eq!(sample(&edge, Point::new(1.4, 0.4, -0.4)), Some(8.0));
        assert_eq!(
            sample(&edge, Point::new(0.5, 0.0, 0.0)),
            Some(6.0),
            "one voxel thick"
        );
    }

    proptest! {
        #[test]
        fn every_voxel_centre_reads_as_its_own_value(
            dims in (1u32..5, 1u32..5, 1u32..5),
            spacing in (0.2f64..5.0, 0.2f64..5.0, 0.2f64..5.0),
            origin in (-50.0f64..50.0, -50.0f64..50.0, -50.0f64..50.0),
            pool in prop::collection::vec(-1000i16..1000, 64),
        ) {
            let (cols, rows, slices) = dims;
            let count = (cols * rows * slices) as usize;
            let values = pool.iter().take(count).copied().map(f32::from).collect();
            let to_patient = Affine::from_columns(
                [spacing.0, 0.0, 0.0],
                [0.0, spacing.1, 0.0],
                [0.0, 0.0, spacing.2],
                [origin.0, origin.1, origin.2],
            );
            let volume = Volume::new([cols, rows, slices], values, to_patient).expect("adds up");
            for slice in 0..slices {
                for row in 0..rows {
                    for col in 0..cols {
                        let voxel = Point::new(f64::from(col), f64::from(row), f64::from(slice));
                        let found = sample(&volume, to_patient.apply(voxel)).expect("inside");
                        let stored = volume.get(col, row, slice).expect("inside");
                        prop_assert!((found - stored).abs() < 1e-3, "{found} for {stored}");
                    }
                }
            }
        }
    }
}
