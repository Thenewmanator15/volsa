use proptest::prelude::*;
use voxa_core::Affine;

use super::*;
use crate::testing::ball;

const DIMS: [u32; 3] = [20, 30, 40];

/// Voxels this far apart along columns, rows and slices, which lie along the patient's x, y
/// and z, with the first voxel's centre at the origin.
fn grid(mm: [f64; 3]) -> Affine<Voxel, Patient> {
    let [across, down, between] = mm;
    Affine::from_columns(
        [across, 0.0, 0.0],
        [0.0, down, 0.0],
        [0.0, 0.0, between],
        [0.0; 3],
    )
}

/// The same grid turned 20 degrees about the patient's x.
fn tilted() -> Affine<Voxel, Patient> {
    let (sin, cos) = 20.0_f64.to_radians().sin_cos();
    Affine::from_columns([1.0, 0.0, 0.0], [0.0, cos, sin], [0.0, -sin, cos], [0.0; 3])
}

fn through() -> Point<Patient> {
    Point::new(5.0, 6.0, 7.0)
}

fn plane(placement: &Affine<Voxel, Patient>, kind: PlaneKind, radiological: bool) -> Plane {
    plane_of(placement, DIMS, kind, through(), radiological).expect("a plane")
}

fn close(a: [f64; 3], b: [f64; 3]) -> bool {
    a.into_iter().zip(b).all(|(a, b)| (a - b).abs() < 1.0e-12)
}

#[test]
fn the_three_kinds_are_named_and_listed_in_one_order() {
    let names = PlaneKind::ALL.map(PlaneKind::name);
    assert_eq!(names, ["Axial", "Coronal", "Sagittal"]);
}

#[test]
fn the_planes_of_a_straight_grid_face_as_radiology_shows_them() {
    let straight = grid([1.0; 3]);
    let axial = plane(&straight, PlaneKind::Axial, true);
    assert_eq!(
        (axial.across, axial.down),
        ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
    );
    assert_eq!(axial.centre, Point::new(9.5, 14.5, 7.0));
    let coronal = plane(&straight, PlaneKind::Coronal, true);
    assert_eq!(
        (coronal.across, coronal.down),
        ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0])
    );
    assert_eq!(coronal.centre, Point::new(9.5, 6.0, 19.5));
    let sagittal = plane(&straight, PlaneKind::Sagittal, true);
    assert_eq!(
        (sagittal.across, sagittal.down),
        ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0])
    );
    assert_eq!(sagittal.centre, Point::new(5.0, 14.5, 19.5));
}

#[test]
fn with_radiological_off_left_and_right_change_places_but_not_in_the_sagittal_plane() {
    let straight = grid([1.0; 3]);
    let across = |kind| plane(&straight, kind, false).across;
    assert_eq!(across(PlaneKind::Axial), [-1.0, 0.0, 0.0]);
    assert_eq!(across(PlaneKind::Coronal), [-1.0, 0.0, 0.0]);
    let (on, off) = (
        plane(&straight, PlaneKind::Sagittal, true),
        plane(&straight, PlaneKind::Sagittal, false),
    );
    assert_eq!(on, off);
    let down = |kind| plane(&straight, kind, false).down;
    assert_eq!(
        down(PlaneKind::Axial),
        [0.0, 1.0, 0.0],
        "front stays at the top"
    );
}

#[test]
fn a_volume_scanned_sagittally_has_the_same_three_planes() {
    // Columns run to the back of the head, rows to the feet, slices to the patient's left.
    let sagittal_scan =
        Affine::from_columns([0.0, 1.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0; 3]);
    let straight = grid([1.0; 3]);
    for kind in PlaneKind::ALL {
        let (scanned, plain) = (
            plane(&sagittal_scan, kind, true),
            plane(&straight, kind, true),
        );
        assert!(close(scanned.across, plain.across), "{kind:?} across");
        assert!(close(scanned.down, plain.down), "{kind:?} down");
    }
}

#[test]
fn a_tilted_volume_has_tilted_planes_along_its_own_axes() {
    let (sin, cos) = 20.0_f64.to_radians().sin_cos();
    let axial = plane(&tilted(), PlaneKind::Axial, true);
    assert!(
        close(axial.normal(), [0.0, -sin, cos]),
        "{:?}",
        axial.normal()
    );
    assert!(close(axial.across, [1.0, 0.0, 0.0]));
    assert!(close(axial.down, [0.0, cos, sin]));
}

#[test]
fn a_placement_that_cannot_be_turned_round_has_no_planes() {
    let flat = Affine::from_columns([0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3]);
    for kind in PlaneKind::ALL {
        assert_eq!(plane_of(&flat, DIMS, kind, through(), true), None);
    }
}

#[test]
fn a_point_is_found_on_a_plane_and_off_it() {
    let coronal = plane(&tilted(), PlaneKind::Coronal, true);
    let ([across, down], off) = coronal.of(coronal.at([3.0, -2.0]));
    assert!((across - 3.0).abs() < 1.0e-12 && (down + 2.0).abs() < 1.0e-12 && off.abs() < 1.0e-12);
    let [x, y, z] = coronal.normal();
    let on = coronal.at([3.0, -2.0]);
    let lifted = Point::new(on.x + 4.0 * x, on.y + 4.0 * y, on.z + 4.0 * z);
    assert!((coronal.of(lifted).1 - 4.0).abs() < 1.0e-12);
}

#[test]
fn layers_are_as_far_apart_as_the_voxels_along_a_direction() {
    let fine = grid([0.5, 0.5, 3.0]);
    assert_eq!(pitch_along(&fine, [0.0, 0.0, 1.0]), Some(3.0));
    assert_eq!(
        pitch_along(&fine, [0.0, 0.0, -7.0]),
        Some(3.0),
        "only the way counts"
    );
    assert_eq!(pitch_along(&fine, [1.0, 0.0, 0.0]), Some(0.5));
    let slanted = pitch_along(&fine, [0.6, 0.0, 0.8]).expect("a pitch");
    assert!((slanted - 0.5 / 0.6).abs() < 1.0e-12, "{slanted}");
    assert_eq!(pitch_along(&fine, [0.0; 3]), None);
    assert_eq!(pitch_along(&fine, [f64::NAN, 0.0, 0.0]), None);
}

#[test]
fn a_point_is_held_inside_the_volume() {
    let straight = grid([1.0; 3]);
    let held = |x, y, z| clamped_into(&straight, DIMS, Point::new(x, y, z));
    assert_eq!(held(-50.0, 6.0, 7.0), Point::new(-0.5, 6.0, 7.0));
    assert_eq!(held(5.0, 6.0, 7.0), Point::new(5.0, 6.0, 7.0));
    assert_eq!(held(500.0, 500.0, 500.0), Point::new(19.5, 29.5, 39.5));
    assert_eq!(
        held(f64::NAN, 6.0, 7.0),
        Point::new(9.5, 14.5, 19.5),
        "nowhere is the middle"
    );
}

#[test]
fn the_slice_nearest_a_point_is_found_and_stops_at_the_ends() {
    let straight = grid([1.0; 3]);
    let nearest = |z| nearest_slice(&straight, 40, Point::new(5.0, 6.0, z));
    assert_eq!((nearest(7.4), nearest(99.0), nearest(-3.0)), (7, 39, 0));
    assert_eq!(nearest(f64::NAN), 0);
    assert_eq!(
        nearest_slice(&straight, 0, through()),
        0,
        "a series of no slices"
    );
}

#[test]
fn the_voxel_in_the_middle_of_a_volume_is_a_voxel_and_not_a_place_between_two() {
    let straight = grid([1.0; 3]);
    assert_eq!(central_voxel(&straight, DIMS), Point::new(10.0, 15.0, 20.0));
    assert_eq!(
        central_voxel(&straight, [21, 31, 1]),
        Point::new(10.0, 15.0, 0.0)
    );
    assert_eq!(
        central_voxel(&straight, [0, 0, 0]),
        Point::new(0.0, 0.0, 0.0)
    );
}

#[test]
fn scrolling_goes_layer_by_layer_the_way_voxel_numbers_rise_and_stops_at_the_ends() {
    let fine = grid([0.5, 0.5, 3.0]);
    let on_slice_7 = Point::new(5.0, 6.0, 21.0);
    let up = [0.0, 0.0, 1.0];
    let to = |z| Some(Point::new(5.0, 6.0, z));
    assert_eq!(scrolled(&fine, DIMS, on_slice_7, up, 1), to(24.0));
    let down = [0.0, 0.0, -1.0];
    assert_eq!(
        scrolled(&fine, DIMS, on_slice_7, down, 1),
        to(24.0),
        "whichever way it faces"
    );
    assert_eq!(scrolled(&fine, DIMS, on_slice_7, up, -2), to(15.0));
    assert_eq!(
        scrolled(&fine, DIMS, on_slice_7, up, 1000),
        to(117.0),
        "the last of 40"
    );
    assert_eq!(scrolled(&fine, DIMS, on_slice_7, up, -1000), to(0.0));
    // From between two layers, a step lands on the layer ahead.
    let between = Point::new(5.0, 6.0, 22.0);
    assert_eq!(scrolled(&fine, DIMS, between, up, 1), to(24.0));
    assert_eq!(scrolled(&fine, DIMS, between, up, -1), to(21.0));
    assert_eq!(scrolled(&fine, DIMS, between, up, 0), Some(between));
    // Across the stack it is the columns that are stepped through.
    let across = scrolled(&fine, DIMS, on_slice_7, [1.0, 0.0, 0.0], 1);
    assert_eq!(across, Some(Point::new(5.5, 6.0, 21.0)));
    assert_eq!(scrolled(&fine, DIMS, on_slice_7, [0.0; 3], 1), None);
    let flat = Affine::from_columns([0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3]);
    assert_eq!(scrolled(&flat, DIMS, on_slice_7, up, 1), None);
}

#[test]
fn a_plane_is_read_out_of_a_volume_with_what_lies_outside_marked() {
    let volume = ball();
    let middle = Point::new(10.0, 10.0, 10.0);
    let placement = volume.voxel_to_patient();
    let axial = plane_of(&placement, volume.dims(), PlaneKind::Axial, middle, true).expect("one");
    let small = reslice(&volume, &axial, [21, 21], 1.0);
    assert_eq!(small.len(), 21 * 21);
    assert_eq!((small[10 * 21 + 10], small[10 * 21 + 17]), (100.0, 0.0));
    let large = reslice(&volume, &axial, [41, 41], 1.0);
    assert!(large[0].is_nan(), "20 mm outside the volume");
    assert_eq!(large[20 * 41 + 20], 100.0);
    assert_eq!(reslice(&volume, &axial, [0, 5], 1.0), Vec::<f32>::new());
}

proptest! {
    #[test]
    fn a_plane_asked_for_through_a_point_holds_it(
        at in prop::array::uniform3(-100.0_f64..100.0),
        kind in prop::sample::select(PlaneKind::ALL.to_vec()),
        radiological in any::<bool>(),
    ) {
        let [x, y, z] = at;
        let point = Point::new(x, y, z);
        let plane = plane_of(&tilted(), DIMS, kind, point, radiological).expect("a plane");
        prop_assert!(plane.of(point).1.abs() < 1.0e-9);
        let [across, down] = [plane.across, plane.down];
        let dot: f64 = across.into_iter().zip(down).map(|(a, d)| a * d).sum();
        prop_assert!(dot.abs() < 1.0e-12, "across and down are square");
    }
}
