//! Lines of sight through a volume.

use voxa_core::{Patient, Point, Volume};

use crate::sample::sample;

/// The most steps a ray is followed for, however large the volume.
const MAX_STEPS: u32 = 100_000;
/// How many times the place of a first hit is halved towards the surface.
const REFINEMENTS: u32 = 8;

/// A line of sight in the patient: where it starts and which way it goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    /// Where the ray starts.
    pub origin: Point<Patient>,
    /// The way it goes, as a direction of length 1.
    pub direction: [f64; 3],
}

impl Ray {
    /// The point `mm` millimetres along the ray.
    pub fn at(&self, mm: f64) -> Point<Patient> {
        let [dx, dy, dz] = self.direction;
        let from = self.origin;
        Point::new(from.x + dx * mm, from.y + dy * mm, from.z + dz * mm)
    }
}

/// How far along a ray, in millimetres, it enters a volume and leaves it. The entry is 0 for
/// a ray that starts inside. `None` when the ray misses the volume or the volume is behind it.
pub fn enter_and_leave(volume: &Volume, ray: &Ray) -> Option<(f64, f64)> {
    // In voxel coordinates the volume is a box along the axes, however the slices are tilted.
    let to_voxel = volume.patient_to_voxel();
    let start = to_voxel.apply(ray.origin);
    let next = to_voxel.apply(ray.at(1.0));
    let starts = [start.x, start.y, start.z];
    let steps = [next.x - start.x, next.y - start.y, next.z - start.z];
    let (mut enter, mut leave) = (0.0_f64, f64::INFINITY);
    for ((from, step), count) in starts.into_iter().zip(steps).zip(volume.dims()) {
        let (low, high) = (-0.5, f64::from(count) - 0.5);
        if step.abs() < 1e-12 {
            // Along this axis the ray goes nowhere: it is between the two faces or it is not.
            if !(low..=high).contains(&from) {
                return None;
            }
            continue;
        }
        let (one, other) = ((low - from) / step, (high - from) / step);
        enter = enter.max(one.min(other));
        leave = leave.min(one.max(other));
    }
    (enter <= leave).then_some((enter, leave))
}

/// The first point along a ray where the volume's value is at or above `threshold`: the
/// surface a person would point at. The ray is followed in steps of half the smallest voxel
/// spacing, and the place found is then narrowed down eight times. `None` when the ray meets
/// nothing that bright.
pub fn first_hit(volume: &Volume, ray: &Ray, threshold: f32) -> Option<Point<Patient>> {
    let (enter, leave) = enter_and_leave(volume, ray)?;
    let step = volume
        .spacing_mm()
        .into_iter()
        .fold(f64::INFINITY, f64::min)
        / 2.0;
    if !step.is_finite() || step <= 0.0 {
        return None;
    }
    // Where there is no value, at the very edge, it counts as not bright enough.
    let bright = |mm: f64| sample(volume, ray.at(mm)).is_some_and(|value| value >= threshold);
    let mut before = enter;
    for count in 0..MAX_STEPS {
        let here = (enter + f64::from(count) * step).min(leave);
        if bright(here) {
            return Some(ray.at(narrowed_down(before, here, &bright)));
        }
        if here >= leave {
            break;
        }
        before = here;
    }
    None
}

/// The earliest bright place between a place that is not bright and one that is.
fn narrowed_down(mut dark: f64, mut bright_at: f64, bright: &impl Fn(f64) -> bool) -> f64 {
    for _ in 0..REFINEMENTS {
        let middle = f64::midpoint(dark, bright_at);
        if bright(middle) {
            bright_at = middle;
        } else {
            dark = middle;
        }
    }
    bright_at
}

#[cfg(test)]
mod tests {
    use voxa_core::Affine;

    use super::*;
    use crate::testing::ball;

    fn along_x(from: [f64; 3]) -> Ray {
        let [x, y, z] = from;
        Ray {
            origin: Point::new(x, y, z),
            direction: [1.0, 0.0, 0.0],
        }
    }

    #[test]
    fn a_point_along_a_ray_is_its_origin_moved_along_its_direction() {
        let ray = Ray {
            origin: Point::new(1.0, 2.0, 3.0),
            direction: [0.0, 0.6, 0.8],
        };
        let at = ray.at(10.0);
        assert_eq!((at.x, at.y, at.z), (1.0, 8.0, 11.0));
    }

    #[test]
    fn a_ray_enters_and_leaves_at_the_faces_of_the_volume() {
        let volume = ball();
        assert_eq!(
            enter_and_leave(&volume, &along_x([-5.0, 10.0, 10.0])),
            Some((4.5, 25.5))
        );
        assert_eq!(
            enter_and_leave(&volume, &along_x([10.0, 10.0, 10.0])),
            Some((0.0, 10.5))
        );
    }

    #[test]
    fn a_ray_that_misses_or_points_away_meets_nothing() {
        let volume = ball();
        assert_eq!(enter_and_leave(&volume, &along_x([-5.0, 40.0, 10.0])), None);
        assert_eq!(
            enter_and_leave(&volume, &along_x([30.0, 10.0, 10.0])),
            None,
            "behind it"
        );
    }

    #[test]
    fn a_ray_at_a_slant_enters_through_whichever_face_it_meets_last() {
        let volume = ball();
        let slant = Ray {
            origin: Point::new(-5.0, -0.5, 10.0),
            direction: [0.6, 0.8, 0.0],
        };
        let (enter, leave) = enter_and_leave(&volume, &slant).expect("passes through");
        // It is past x = -0.5 after 7.5 mm, and leaves through y = 20.5 after 26.25 mm.
        assert!(
            (enter - 7.5).abs() < 1e-9 && (leave - 26.25).abs() < 1e-9,
            "{enter} {leave}"
        );
    }

    #[test]
    fn a_tilted_stack_is_entered_where_its_own_faces_are() {
        // Slices 2 mm apart, each 1 mm further along y than the one before it.
        let tilted =
            Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 2.0], [0.0; 3]);
        let volume = Volume::new([4, 4, 4], vec![0.0; 64], tilted).expect("adds up");
        let up = Ray {
            origin: Point::new(1.0, 1.0, -10.0),
            direction: [0.0, 0.0, 1.0],
        };
        let (enter, leave) = enter_and_leave(&volume, &up).expect("passes through");
        // The ray is in slice z / 2 and row 1 - z / 2: it comes in through the first slice at
        // z = -1 and goes out through the first row at z = 3. A box laid along the patient's
        // axes would have let it out at z = 7.
        assert!(
            (enter - 9.0).abs() < 1e-9 && (leave - 13.0).abs() < 1e-9,
            "{enter} {leave}"
        );
    }

    #[test]
    fn the_first_hit_is_where_the_value_first_reaches_the_threshold() {
        let volume = ball();
        let hit = first_hit(&volume, &along_x([-5.0, 10.0, 10.0]), 50.0).expect("hits the ball");
        assert!((hit.x - 6.5).abs() < 0.05, "{hit:?}");
        assert_eq!((hit.y, hit.z), (10.0, 10.0));
    }

    #[test]
    fn the_first_hit_is_narrowed_down_to_well_within_a_step() {
        let volume = ball();
        // The value climbs from 0 at x = 6 to 100 at x = 7, so it passes 20 at x = 6.2.
        let hit = first_hit(&volume, &along_x([-5.0, 10.0, 10.0]), 20.0).expect("hits the ball");
        assert!((hit.x - 6.2).abs() < 0.005, "{hit:?}");
    }

    #[test]
    fn a_ray_past_the_ball_or_a_threshold_too_high_hits_nothing() {
        let volume = ball();
        assert_eq!(first_hit(&volume, &along_x([-5.0, 2.0, 2.0]), 50.0), None);
        assert_eq!(
            first_hit(&volume, &along_x([-5.0, 10.0, 10.0]), 500.0),
            None
        );
        assert_eq!(
            first_hit(&volume, &along_x([-5.0, 40.0, 10.0]), 50.0),
            None,
            "misses"
        );
    }

    #[test]
    fn a_ray_starting_in_bright_tissue_hits_at_once() {
        let volume = ball();
        let hit = first_hit(&volume, &along_x([10.0, 10.0, 10.0]), 50.0).expect("already inside");
        assert_eq!((hit.x, hit.y, hit.z), (10.0, 10.0, 10.0));
    }
}
