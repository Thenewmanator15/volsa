use std::f64::consts::PI;

use volsa_core::{Point, Volume};

use super::*;
use crate::plane::{PlaneKind, plane_of};
use crate::testing::{ball, filled};

fn at(x: f64, y: f64, z: f64) -> Point<Patient> {
    Point::new(x, y, z)
}

/// The axial plane through the middle of a 21-voxel test volume.
fn axial(volume: &Volume) -> Plane {
    let placement = volume.voxel_to_patient();
    let middle = at(10.0, 10.0, 10.0);
    plane_of(&placement, volume.dims(), PlaneKind::Axial, middle, true).expect("a plane")
}

#[test]
fn a_distance_is_the_straight_line_between_two_points() {
    assert_eq!(distance_mm(at(0.0, 0.0, 0.0), at(3.0, 4.0, 0.0)), 5.0);
    assert_eq!(distance_mm(at(0.0, 0.0, 0.0), at(2.0, 3.0, 6.0)), 7.0);
    assert_eq!(distance_mm(at(1.0, 1.0, 1.0), at(1.0, 1.0, 1.0)), 0.0);
}

#[test]
fn an_angle_is_measured_at_its_vertex_from_0_to_180_degrees() {
    let vertex = at(1.0, 1.0, 1.0);
    let angle = |a: [f64; 3], b: [f64; 3]| {
        let end = |[x, y, z]: [f64; 3]| at(1.0 + x, 1.0 + y, 1.0 + z);
        angle_degrees(end(a), vertex, end(b))
    };
    let near = |found: Option<f64>, wanted: f64| found.is_some_and(|f| (f - wanted).abs() < 1.0e-9);
    assert!(near(angle([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), 90.0));
    assert!(near(angle([1.0, 0.0, 0.0], [-3.0, 0.0, 0.0]), 180.0));
    assert!(near(angle([1.0, 0.0, 0.0], [5.0, 0.0, 0.0]), 0.0));
    assert!(near(angle([1.0, 0.0, 0.0], [1.0, 1.0, 0.0]), 45.0));
    assert_eq!(
        angle([0.0; 3], [0.0, 1.0, 0.0]),
        None,
        "an arm of no length"
    );
    assert_eq!(angle([1.0, 0.0, 0.0], [f64::NAN, 0.0, 0.0]), None);
}

#[test]
fn a_region_of_one_value_has_that_value_and_no_spread() {
    let even = filled([21; 3], |_, _, _| 7.0);
    let stats = ellipse_stats(&even, &axial(&even), [0.0, 0.0], [4.0, 2.0]).expect("inside");
    assert_eq!(
        (stats.mean, stats.sd, stats.min, stats.max),
        (7.0, 0.0, 7.0, 7.0)
    );
    assert!(
        (stats.area_mm2 - 8.0 * PI).abs() < 1.0e-9,
        "{}",
        stats.area_mm2
    );
    // Sampled every half millimetre: about four samples to the square millimetre.
    let expected = 4.0 * 8.0 * PI;
    assert!(
        (stats.samples as f64 - expected).abs() < expected * 0.1,
        "{}",
        stats.samples
    );
}

#[test]
fn a_region_is_measured_from_the_scan_values_inside_it() {
    let volume = ball();
    let plane = axial(&volume);
    let inside = ellipse_stats(&volume, &plane, [0.0, 0.0], [2.0, 2.0]).expect("inside");
    assert_eq!((inside.mean, inside.sd), (100.0, 0.0));
    let wide = ellipse_stats(&volume, &plane, [0.0, 0.0], [8.0, 8.0]).expect("inside");
    assert!((10.0..20.0).contains(&wide.mean), "{}", wide.mean);
    assert!(wide.sd > 30.0, "{}", wide.sd);
    assert_eq!((wide.min, wide.max), (0.0, 100.0));
}

#[test]
fn a_region_with_no_size_or_wholly_outside_the_scan_has_no_numbers() {
    let volume = ball();
    let plane = axial(&volume);
    assert_eq!(ellipse_stats(&volume, &plane, [0.0, 0.0], [0.0, 2.0]), None);
    assert_eq!(
        ellipse_stats(&volume, &plane, [0.0, 0.0], [2.0, -1.0]),
        None
    );
    assert_eq!(
        ellipse_stats(&volume, &plane, [0.0, 0.0], [f64::NAN, 2.0]),
        None
    );
    assert_eq!(
        ellipse_stats(&volume, &plane, [40.0, 0.0], [3.0, 3.0]),
        None
    );
}

#[test]
fn a_region_half_outside_the_scan_counts_only_what_is_inside() {
    let volume = ball();
    let plane = axial(&volume);
    let whole = ellipse_stats(&volume, &plane, [0.0, 0.0], [3.0, 3.0]).expect("inside");
    // The volume ends 10.5 mm to the side of its middle.
    let half = ellipse_stats(&volume, &plane, [-10.5, 0.0], [3.0, 3.0]).expect("half inside");
    let share = half.samples as f64 / whole.samples as f64;
    assert!((0.4..0.6).contains(&share), "{share}");
    assert_eq!(
        half.area_mm2, whole.area_mm2,
        "the area is that of the ellipse drawn"
    );
}

#[test]
fn a_very_large_region_is_sampled_more_coarsely_not_for_ever() {
    let even = filled([21; 3], |_, _, _| 7.0);
    let stats = ellipse_stats(&even, &axial(&even), [0.0, 0.0], [5.0e4, 5.0e4]).expect("some");
    assert_eq!(stats.mean, 7.0);
    assert!(stats.samples < 2_000_000, "{}", stats.samples);
}
