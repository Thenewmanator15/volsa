use std::f64::consts::FRAC_PI_2;

use proptest::prelude::*;

use super::*;

const VIEWS: [NamedView; 6] = [
    NamedView::Anterior,
    NamedView::Posterior,
    NamedView::Left,
    NamedView::Right,
    NamedView::Superior,
    NamedView::Inferior,
];

/// A camera facing one side of a patient whose origin is in the middle, at 1 mm a pixel.
fn from(view: NamedView) -> Camera {
    Camera::looking(view, Point::new(0.0, 0.0, 0.0), 1.0)
}

fn close(found: [f64; 3], expected: [f64; 3]) -> bool {
    found
        .into_iter()
        .zip(expected)
        .all(|(found, expected)| (found - expected).abs() < 1e-12)
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let ([ax, ay, az], [bx, by, bz]) = (a, b);
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

/// A box this many millimetres along x, y and z, centred on the patient origin.
fn box_of(size: [f64; 3]) -> [Point<Patient>; 8] {
    let [x, y, z] = size.map(|side| side / 2.0);
    let end = |far: bool, half: f64| if far { half } else { -half };
    std::array::from_fn(|i| Point::new(end(i & 1 != 0, x), end(i & 2 != 0, y), end(i & 4 != 0, z)))
}

#[test]
fn each_named_view_faces_the_side_it_is_named_for() {
    // The right of the screen, down the screen and away from the viewer, in the patient,
    // where x runs to the patient's left, y to their back and z to their head.
    let expected = [
        (
            NamedView::Anterior,
            [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
        ),
        (
            NamedView::Posterior,
            [[-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0]],
        ),
        (
            NamedView::Left,
            [[0.0, 1.0, 0.0], [0.0, 0.0, -1.0], [-1.0, 0.0, 0.0]],
        ),
        (
            NamedView::Right,
            [[0.0, -1.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]],
        ),
        (
            NamedView::Superior,
            [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
        ),
        (
            NamedView::Inferior,
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        ),
    ];
    for (view, axes) in expected {
        for (found, expected) in from(view).axes().into_iter().zip(axes) {
            assert!(
                close(found, expected),
                "{view:?}: {found:?} for {expected:?}"
            );
        }
    }
}

#[test]
fn no_named_view_is_mirrored() {
    for view in VIEWS {
        let [right, down, forward] = from(view).axes();
        assert!(close(cross(right, down), forward), "{view:?}");
    }
}

#[test]
fn the_screen_is_laid_over_the_patient_about_the_target() {
    let to_patient = from(NamedView::Anterior).screen_to_patient([200.0, 100.0]);
    let at = |x, y, z| {
        let found = to_patient.apply(Point::new(x, y, z));
        [found.x, found.y, found.z]
    };
    assert!(
        close(at(100.0, 50.0, 0.0), [0.0, 0.0, 0.0]),
        "the middle is the target"
    );
    assert!(
        close(at(110.0, 50.0, 0.0), [10.0, 0.0, 0.0]),
        "right is the patient's left"
    );
    assert!(
        close(at(100.0, 60.0, 0.0), [0.0, 0.0, -10.0]),
        "down is towards the feet"
    );
    assert!(
        close(at(100.0, 50.0, 5.0), [0.0, 5.0, 0.0]),
        "beyond the target is further back"
    );
}

#[test]
fn the_scale_sets_how_many_millimetres_a_pixel_covers() {
    let camera = Camera::looking(NamedView::Anterior, Point::new(5.0, 6.0, 7.0), 2.0);
    let found = camera
        .screen_to_patient([200.0, 100.0])
        .apply(Point::new(110.0, 40.0, 0.0));
    assert!(
        close([found.x, found.y, found.z], [25.0, 6.0, 27.0]),
        "{found:?}"
    );
}

#[test]
fn dragging_across_turns_the_patient_with_the_pointer() {
    // Dragged a quarter turn to the right, the patient's right side comes round to the front.
    let turned = from(NamedView::Anterior).orbited([180.0, 0.0]);
    for (found, expected) in turned.axes().into_iter().zip(from(NamedView::Right).axes()) {
        assert!(close(found, expected), "{found:?} for {expected:?}");
    }
    let back = turned.orbited([-180.0, 0.0]);
    assert!(close(back.axes()[2], [0.0, 1.0, 0.0]), "and back again");
}

#[test]
fn dragging_down_tips_the_view_onto_the_head_and_no_further() {
    let tipped = from(NamedView::Anterior).orbited([0.0, 180.0]);
    assert!(
        close(tipped.axes()[2], [0.0, 0.0, -1.0]),
        "looking straight down"
    );
    assert_eq!(
        from(NamedView::Anterior).orbited([0.0, 1000.0]).pitch,
        FRAC_PI_2
    );
    assert_eq!(
        from(NamedView::Anterior).orbited([0.0, -1000.0]).pitch,
        -FRAC_PI_2
    );
}

#[test]
fn the_yaw_stays_within_one_turn_however_far_the_view_is_spun() {
    let spun = from(NamedView::Anterior).orbited([1.0e6, 0.0]);
    assert!(
        (0.0..std::f64::consts::TAU).contains(&spun.yaw),
        "{}",
        spun.yaw
    );
}

#[test]
fn panning_moves_the_scene_with_the_pointer() {
    let camera = Camera::looking(NamedView::Anterior, Point::new(0.0, 0.0, 0.0), 2.0);
    let across = camera.panned([10.0, 0.0]).target;
    assert!(
        close([across.x, across.y, across.z], [-20.0, 0.0, 0.0]),
        "{across:?}"
    );
    let down = camera.panned([0.0, 10.0]).target;
    assert!(
        close([down.x, down.y, down.z], [0.0, 0.0, 20.0]),
        "{down:?}"
    );
}

#[test]
fn zooming_keeps_the_point_under_the_pointer_still() {
    let zoomed = from(NamedView::Anterior).zoomed_about(2.0, [50.0, 0.0]);
    assert_eq!(zoomed.mm_per_px, 0.5);
    assert!(close(
        [zoomed.target.x, zoomed.target.y, zoomed.target.z],
        [25.0, 0.0, 0.0]
    ));

    let camera =
        Camera::looking(NamedView::Left, Point::new(3.0, -4.0, 5.0), 0.8).orbited([33.0, -21.0]);
    let (viewport, pointer) = ([640.0, 480.0], Point::new(320.0 + 90.0, 240.0 - 70.0, 0.0));
    let before = camera.screen_to_patient(viewport).apply(pointer);
    let after = camera
        .zoomed_about(1.7, [90.0, -70.0])
        .screen_to_patient(viewport)
        .apply(pointer);
    assert!(
        close([after.x, after.y, after.z], [before.x, before.y, before.z]),
        "{after:?}"
    );
}

#[test]
fn zooming_stops_at_its_limits() {
    let camera = from(NamedView::Anterior);
    assert_eq!(
        camera.zoomed_about(1.0e9, [0.0, 0.0]).mm_per_px,
        MIN_MM_PER_PX
    );
    assert_eq!(
        camera.zoomed_about(1.0e-9, [0.0, 0.0]).mm_per_px,
        MAX_MM_PER_PX
    );
}

#[test]
fn a_movement_that_is_not_a_number_changes_nothing() {
    let camera = from(NamedView::Left);
    assert_eq!(camera.zoomed_about(f32::NAN, [0.0, 0.0]), camera);
    assert_eq!(camera.zoomed_about(0.0, [0.0, 0.0]), camera);
    assert_eq!(camera.zoomed_about(-2.0, [0.0, 0.0]), camera);
    assert_eq!(camera.zoomed_about(2.0, [f32::NAN, 0.0]), camera);
    assert_eq!(camera.orbited([f32::NAN, 0.0]), camera);
    assert_eq!(camera.panned([0.0, f32::INFINITY]), camera);
}

#[test]
fn a_fitted_camera_shows_the_whole_box_in_the_middle() {
    let corners = box_of([100.0, 200.0, 50.0]);
    let front = Camera::fitted(NamedView::Anterior, &corners, [400.0, 400.0]);
    assert!(
        (front.mm_per_px - 100.0 / 360.0).abs() < 1e-9,
        "{}",
        front.mm_per_px
    );
    assert!(close(
        [front.target.x, front.target.y, front.target.z],
        [0.0, 0.0, 0.0]
    ));
    let side = Camera::fitted(NamedView::Left, &corners, [400.0, 400.0]);
    assert!(
        (side.mm_per_px - 200.0 / 360.0).abs() < 1e-9,
        "{}",
        side.mm_per_px
    );
    let wide = Camera::fitted(NamedView::Anterior, &corners, [400.0, 100.0]);
    assert!(
        (wide.mm_per_px - 50.0 / 90.0).abs() < 1e-9,
        "the height is the tighter side"
    );
}

#[test]
fn a_fitted_camera_is_centred_on_a_box_that_is_not_at_the_origin() {
    let corners = box_of([10.0, 10.0, 10.0]).map(|at| Point::new(at.x + 30.0, at.y - 40.0, at.z));
    let camera = Camera::fitted(NamedView::Superior, &corners, [100.0, 100.0]);
    assert!(close(
        [camera.target.x, camera.target.y, camera.target.z],
        [30.0, -40.0, 0.0]
    ));
    let nothing = [Point::new(1.0, 2.0, 3.0); 8];
    let closest = Camera::fitted(NamedView::Anterior, &nothing, [0.0, f32::NAN]);
    assert_eq!(
        closest.mm_per_px, MIN_MM_PER_PX,
        "a box of no size is still given a view"
    );
}

#[test]
fn a_line_of_sight_starts_on_the_viewers_side_and_goes_straight_ahead() {
    let camera = Camera::looking(NamedView::Anterior, Point::new(5.0, 6.0, 7.0), 2.0);
    let middle = camera.ray_through([200.0, 100.0], [100.0, 50.0]);
    assert_eq!(middle.direction, [0.0, 1.0, 0.0]);
    assert!(
        close([middle.origin.x, 0.0, middle.origin.z], [5.0, 0.0, 7.0]),
        "{middle:?}"
    );
    assert!(
        middle.origin.y < 6.0 - 1000.0,
        "well in front of the patient: {middle:?}"
    );
    let aside = camera.ray_through([200.0, 100.0], [110.0, 50.0]);
    assert!((aside.origin.x - 25.0).abs() < 1e-9, "{aside:?}");
    assert_eq!(
        aside.direction, middle.direction,
        "lines of sight are parallel"
    );
}

#[test]
fn a_camera_with_numbers_that_cannot_be_used_still_gives_a_view() {
    let broken = Camera {
        target: Point::new(f64::NAN, 0.0, f64::INFINITY),
        yaw: f64::NAN,
        pitch: f64::INFINITY,
        mm_per_px: 0.0,
    };
    assert!(
        broken
            .screen_to_patient([f32::NAN, 0.0])
            .inverse()
            .is_some()
    );
    assert!(
        close(broken.axes()[0], [1.0, 0.0, 0.0]),
        "read as seen from the front"
    );
    let moved = broken
        .orbited([10.0, 0.0])
        .panned([1.0, 1.0])
        .zoomed_about(2.0, [3.0, 4.0]);
    assert!(moved.yaw.is_finite() && moved.pitch.is_finite() && moved.mm_per_px > 0.0);
    assert!(
        moved.target.x.is_finite() && moved.target.z.is_finite(),
        "{moved:?}"
    );
}

proptest! {
    #[test]
    fn a_point_on_the_screen_can_always_be_found_again(
        yaw in -20.0f64..20.0,
        pitch in -FRAC_PI_2..FRAC_PI_2,
        mm_per_px in MIN_MM_PER_PX..MAX_MM_PER_PX,
        target in (-500.0f64..500.0, -500.0f64..500.0, -500.0f64..500.0),
        viewport in (1.0f32..4000.0, 1.0f32..4000.0),
        at in (-5000.0f64..5000.0, -5000.0f64..5000.0, -5000.0f64..5000.0),
    ) {
        let camera =
            Camera { target: Point::new(target.0, target.1, target.2), yaw, pitch, mm_per_px };
        let to_patient = camera.screen_to_patient([viewport.0, viewport.1]);
        let to_screen = to_patient.inverse().expect("can be undone");
        let found = to_screen.apply(to_patient.apply(Point::new(at.0, at.1, at.2)));
        let apart = (found.x - at.0).abs() + (found.y - at.1).abs() + (found.z - at.2).abs();
        prop_assert!(apart < 1e-6, "{found:?} for {at:?}");

        let [right, down, forward] = camera.axes();
        prop_assert!(close(cross(right, down), forward), "mirrored at {yaw} {pitch}");
    }
}
