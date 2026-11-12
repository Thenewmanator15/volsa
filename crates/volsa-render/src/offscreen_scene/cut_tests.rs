//! Pictures of a volume cut open, and of the 2D slice drawn inside it.

use volsa_core::Point;

use super::tests::{CORE, IN_THE_SHELL, MIDDLE, SHELL, at, drawn};
use super::*;
use crate::camera::NamedView;
use crate::scene::{CutPlane, Look, SlicePlane};
use crate::testing::{ball, brightest, headless_scene, scene_of};

/// Pixels 10 mm to the right and to the left of the middle of a fitted picture of the ball.
const RIGHT_OF_MIDDLE: (u32, u32) = (66, 48);
const LEFT_OF_MIDDLE: (u32, u32) = (30, 48);

fn cut(point: [f64; 3], normal: [f64; 3]) -> CutPlane {
    let [x, y, z] = point;
    CutPlane {
        point: Point::new(x, y, z),
        normal,
    }
}

/// A cut through the middle that removes the front half of the patient.
fn front_half_off() -> CutPlane {
    cut([0.0; 3], [0.0, -1.0, 0.0])
}

/// A cut through the middle that removes the patient's right half.
fn right_half_off() -> CutPlane {
    cut([0.0; 3], [-1.0, 0.0, 0.0])
}

/// The ball drawn from one side in one look, the scene having been changed by `change`.
/// `None` with no adapter.
fn ball_with(
    view: NamedView,
    look: Look,
    change: impl FnOnce(&mut SceneParams),
) -> Option<Vec<u8>> {
    let mut gpu = headless_scene()?;
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    let mut scene = scene_of(&volume, view, look);
    change(&mut scene);
    Some(drawn(&mut gpu, &scene))
}

fn near(found: u8, expected: u8) -> bool {
    found.abs_diff(expected) <= 3
}

#[test]
fn a_cut_face_shows_the_scan_as_a_slice_would() {
    let cut_open = |scene: &mut SceneParams| scene.cuts = vec![front_half_off()];
    let Some(rgba) = ball_with(NamedView::Anterior, Look::Surface, cut_open) else {
        return;
    };
    assert!(
        near(at(&rgba, MIDDLE), CORE),
        "the core, unlit: {}",
        at(&rgba, MIDDLE)
    );
    let beside = at(&rgba, RIGHT_OF_MIDDLE);
    assert!(near(beside, SHELL), "the shell, unlit: {beside}");
    assert_eq!(
        rgba[..4],
        [0, 0, 0, 255],
        "outside the volume is still black"
    );
}

#[test]
fn where_a_cut_passes_through_air_the_surface_behind_it_is_drawn_lit() {
    // A cut 10 mm in front of the middle is inside the ball in the middle of the picture
    // and in the air towards the ball's edge, where the ball begins a little further back.
    let shallow = cut([0.0, -10.0, 0.0], [0.0, -1.0, 0.0]);
    let cut_open = |scene: &mut SceneParams| scene.cuts = vec![shallow];
    let Some(rgba) = ball_with(NamedView::Anterior, Look::Surface, cut_open) else {
        return;
    };
    assert!(
        near(at(&rgba, MIDDLE), SHELL),
        "the cut face: {}",
        at(&rgba, MIDDLE)
    );
    let behind = at(&rgba, IN_THE_SHELL);
    assert!(
        behind > 0 && behind < SHELL - 8,
        "a surface turned half away: {behind}"
    );
}

#[test]
fn a_second_cut_removes_more() {
    let cut_open = |scene: &mut SceneParams| scene.cuts = vec![front_half_off(), right_half_off()];
    let Some(rgba) = ball_with(NamedView::Anterior, Look::Surface, cut_open) else {
        return;
    };
    let kept = at(&rgba, RIGHT_OF_MIDDLE);
    assert!(
        near(kept, SHELL),
        "the patient's left is still there: {kept}"
    );
    assert_eq!(at(&rgba, LEFT_OF_MIDDLE), 0, "the patient's right is gone");
}

#[test]
fn a_fourth_cut_changes_nothing() {
    let top_off = cut([0.0, 0.0, 10.0], [0.0, 0.0, 1.0]);
    let three = vec![front_half_off(), right_half_off(), top_off];
    // A cut that would take the back half too, and leave nothing at all.
    let four = [three.clone(), vec![cut([0.0; 3], [0.0, 1.0, 0.0])]].concat();
    let with = |cuts: Vec<CutPlane>| {
        ball_with(NamedView::Anterior, Look::Surface, |scene| {
            scene.cuts = cuts;
        })
    };
    let (Some(with_three), Some(with_four)) = (with(three), with(four)) else {
        return;
    };
    assert!(
        near(at(&with_three, RIGHT_OF_MIDDLE), SHELL),
        "a quarter of the ball is left"
    );
    assert_eq!(
        at(&with_three, (66, 24)),
        0,
        "the third cut took the top off"
    );
    assert_eq!(with_four, with_three);
}

#[test]
fn a_cut_only_clips_the_brightest_value_look() {
    let cut_open = |scene: &mut SceneParams| scene.cuts = vec![front_half_off()];
    let Some(rgba) = ball_with(NamedView::Anterior, Look::Mip, cut_open) else {
        return;
    };
    assert!(
        near(at(&rgba, MIDDLE), CORE),
        "the core: {}",
        at(&rgba, MIDDLE)
    );
    assert!(
        near(at(&rgba, IN_THE_SHELL), SHELL),
        "the shell: {}",
        at(&rgba, IN_THE_SHELL)
    );
    assert!(
        brightest(&rgba) <= CORE + 3,
        "the bead went with the front half"
    );
}

#[test]
fn a_cut_plane_with_no_direction_is_left_out() {
    let odd = vec![cut([0.0; 3], [0.0; 3]), cut([0.0; 3], [f64::NAN, 0.0, 0.0])];
    let change = |scene: &mut SceneParams| scene.cuts = odd;
    let Some(with) = ball_with(NamedView::Anterior, Look::Surface, change) else {
        return;
    };
    let Some(without) = ball_with(NamedView::Anterior, Look::Surface, |_| {}) else {
        return;
    };
    assert_eq!(with, without);
}

#[test]
fn the_slice_plane_is_an_opaque_sheet_showing_the_scan() {
    let Some(without) = ball_with(NamedView::Inferior, Look::Mip, |_| {}) else {
        return;
    };
    assert!(
        near(at(&without, MIDDLE), CORE),
        "the core, with no slice in the way"
    );
    // Seen from below, a slice 10 mm below the middle is in front of the core and the bead.
    let sheet = SlicePlane {
        point: Point::new(0.0, 0.0, -10.0),
        normal: [0.0, 0.0, 1.0],
    };
    let add = |scene: &mut SceneParams| scene.slice = Some(sheet);
    let Some(with) = ball_with(NamedView::Inferior, Look::Mip, add) else {
        return;
    };
    assert!(
        near(at(&with, MIDDLE), SHELL),
        "the shell on the slice: {}",
        at(&with, MIDDLE)
    );
    assert!(
        brightest(&with) <= 70,
        "nothing behind the slice: {}",
        brightest(&with)
    );
}

#[test]
fn the_slice_plane_hides_what_is_behind_it_even_where_the_scan_is_empty() {
    // Seen from behind, a slice 20 mm behind the middle is in the air in front of the ball.
    let sheet = SlicePlane {
        point: Point::new(0.0, 20.0, 0.0),
        normal: [0.0, -1.0, 0.0],
    };
    let add = |scene: &mut SceneParams| scene.slice = Some(sheet);
    let Some(rgba) = ball_with(NamedView::Posterior, Look::Surface, add) else {
        return;
    };
    assert_eq!(brightest(&rgba), 0);
}

#[test]
fn what_is_in_front_of_the_slice_plane_is_drawn_over_it() {
    let sheet = SlicePlane {
        point: Point::new(0.0, 0.0, 0.0),
        normal: [0.0, 1.0, 0.0],
    };
    let middle = |look, with_slice: bool| {
        let change = |scene: &mut SceneParams| scene.slice = with_slice.then_some(sheet);
        ball_with(NamedView::Anterior, look, change).map(|rgba| at(&rgba, MIDDLE))
    };
    // See-through tissue in front lets the slice show, dimmed by what it has to pass.
    let (Some(with), Some(without)) = (middle(Look::Vr, true), middle(Look::Vr, false)) else {
        return;
    };
    assert!(
        with > without && with < CORE,
        "{with} with the slice, {without} without"
    );
    // A solid surface in front hides it.
    let solid = (middle(Look::Surface, true), middle(Look::Surface, false));
    let (Some(with), Some(without)) = solid else {
        return;
    };
    assert!(
        with.abs_diff(without) <= 5,
        "{with} with the slice, {without} without"
    );
}

#[test]
fn a_slice_plane_along_the_line_of_sight_or_with_no_direction_draws_nothing() {
    let Some(without) = ball_with(NamedView::Anterior, Look::Mip, |_| {}) else {
        return;
    };
    for normal in [[1.0, 0.0, 0.0], [0.0; 3]] {
        let sheet = SlicePlane {
            point: Point::new(0.0, 0.0, 0.0),
            normal,
        };
        let add = |scene: &mut SceneParams| scene.slice = Some(sheet);
        let Some(with) = ball_with(NamedView::Anterior, Look::Mip, add) else {
            return;
        };
        assert_eq!(with, without, "{normal:?}");
    }
}
