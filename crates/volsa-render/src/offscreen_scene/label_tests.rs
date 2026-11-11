//! Labels drawn over a plane: tinted, outlined, hidden, faded and selected, and always where
//! the patient has them, whatever grid they are on.

use voxa_core::{Patient, Point, Volume};
use voxa_labels::LabelSet;
use voxa_volume::{PlaneKind, plane_of};

use super::*;
use crate::labels::{FILL, Overlay, PartLook};
use crate::plane_view::PlaneView;
use crate::testing::{
    BLUE, GREEN, RED, SIDE, ball, ball_labels, ball_on, ball_window, centred, headless_scene,
};

/// The middle row of a picture, and the column just to the patient's right of the middle in
/// a radiological axial picture: the last pixel of the right half before the left begins.
const ROW: u32 = 48;
const LAST_OF_RIGHT: u32 = 47;
/// A pixel 8 mm to the patient's left of the middle of that picture: in the left half.
const IN_THE_LEFT: (u32, u32) = (63, ROW);

fn axial(volume: &Volume, radiological: bool) -> PlaneView {
    let placement = volume.voxel_to_patient();
    let middle: Point<Patient> = Point::new(0.0, 0.0, 0.0);
    let plane = plane_of(
        &placement,
        volume.dims(),
        PlaneKind::Axial,
        middle,
        radiological,
    );
    let panel = [SIDE as f32; 2];
    PlaneView::showing(
        plane.expect("a plane"),
        &volume.corners(),
        panel,
        1.0,
        [0.0; 2],
    )
}

fn drawn(gpu: &mut HeadlessScene, view: &PlaneView) -> Vec<u8> {
    gpu.render_plane(view, ball_window(), [SIDE, SIDE])
        .expect("renders")
}

fn rgb(rgba: &[u8], (x, y): (u32, u32)) -> [u8; 3] {
    let at = ((y * SIDE + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2]]
}

/// Every part shown, solid, in the colour its set gives it.
fn all_shown(set: &LabelSet) -> Vec<PartLook> {
    let solid = |part: &voxa_labels::Part| PartLook {
        colour: part.colour,
        opacity: 1.0,
    };
    set.parts.iter().map(solid).collect()
}

/// A colour mixed this far towards another, as the shader mixes them.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a share of 255, held between 0 and 255"
)]
fn mixed(from: [u8; 3], to: [u8; 3], share: f32) -> [u8; 3] {
    let channel =
        |at: usize| f32::from(from[at]) + (f32::from(to[at]) - f32::from(from[at])) * share;
    [0, 1, 2].map(|at| channel(at).round().clamp(0.0, 255.0) as u8)
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.into_iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 2)
}

/// The ball with its labels on the device and every part shown; the plain axial picture.
fn labelled(gpu: &mut HeadlessScene) -> (Volume, LabelSet, Vec<u8>) {
    let (volume, set) = (ball(), ball_labels([0.0; 3]));
    gpu.upload(&volume).expect("uploads");
    let plain = drawn(gpu, &axial(&volume, true));
    assert!(gpu.label(Some(&set)), "there is room for them");
    gpu.set_looks(&all_shown(&set), None);
    (volume, set, plain)
}

#[test]
fn filled_labels_tint_each_half_on_the_side_of_the_picture_its_letter_is() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let (volume, _, plain) = labelled(&mut gpu);
    gpu.overlay(Overlay::Filled);
    let filled = drawn(&mut gpu, &axial(&volume, true));
    let under = rgb(&plain, IN_THE_LEFT);
    let seen = rgb(&filled, IN_THE_LEFT);
    assert!(
        near(seen, mixed(under, RED, FILL)),
        "a third of the way to red: {seen:?} over {under:?}"
    );
    // The other way round, that pixel shows the patient's right, which is the blue half.
    let plain_other = {
        gpu.overlay(Overlay::Off);
        drawn(&mut gpu, &axial(&volume, false))
    };
    gpu.overlay(Overlay::Filled);
    let other = drawn(&mut gpu, &axial(&volume, false));
    let seen = rgb(&other, IN_THE_LEFT);
    assert!(
        near(seen, mixed(rgb(&plain_other, IN_THE_LEFT), BLUE, FILL)),
        "{seen:?}"
    );
    assert!(
        filled.chunks(4).all(|pixel| pixel[3] == 255),
        "every pixel is opaque"
    );
}

#[test]
fn outlined_labels_leave_the_scan_alone_and_draw_the_line_between_parts() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let (volume, _, plain) = labelled(&mut gpu);
    gpu.overlay(Overlay::Outline);
    let outlined = drawn(&mut gpu, &axial(&volume, true));
    assert_eq!(
        rgb(&outlined, IN_THE_LEFT),
        rgb(&plain, IN_THE_LEFT),
        "inside a part: the scan"
    );
    let line = rgb(&outlined, (LAST_OF_RIGHT, ROW));
    assert!(
        near(line, BLUE),
        "between the halves, in the colour of the part it bounds: {line:?}"
    );
    // The bead's box is outlined in its own colour somewhere in the picture, and only a
    // little of the picture is outline.
    let changed = outlined
        .chunks(4)
        .zip(plain.chunks(4))
        .filter(|(a, b)| a != b);
    let lines = changed.clone().count();
    assert!(lines > 100 && lines < 1500, "{lines} pixels of outline");
    assert!(
        changed
            .clone()
            .any(|(pixel, _)| near([pixel[0], pixel[1], pixel[2]], GREEN))
    );
    assert!(
        changed
            .clone()
            .any(|(pixel, _)| near([pixel[0], pixel[1], pixel[2]], RED))
    );
}

#[test]
fn with_the_overlay_off_or_no_labels_the_picture_is_the_plain_one_exactly() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let (volume, _, plain) = labelled(&mut gpu);
    gpu.overlay(Overlay::Off);
    assert_eq!(drawn(&mut gpu, &axial(&volume, true)), plain);
    gpu.overlay(Overlay::Filled);
    assert_ne!(drawn(&mut gpu, &axial(&volume, true)), plain);
    assert!(!gpu.label(None), "nothing to draw from");
    assert_eq!(drawn(&mut gpu, &axial(&volume, true)), plain);
    // Another volume uploaded takes the labels off: they were put there for the one before.
    let set = ball_labels([0.0; 3]);
    assert!(gpu.label(Some(&set)));
    gpu.upload(&volume).expect("uploads");
    assert_eq!(drawn(&mut gpu, &axial(&volume, true)), plain);
}

#[test]
fn a_hidden_part_is_neither_tinted_nor_outlined_except_where_a_shown_one_bounds_it() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let (volume, set, plain) = labelled(&mut gpu);
    gpu.overlay(Overlay::Outline);
    let shown = drawn(&mut gpu, &axial(&volume, true));
    // Along the middle row the left half ends at a red pixel some 16 mm out.
    let rim = (ROW..SIDE)
        .rev()
        .find(|x| near(rgb(&shown, (*x, ROW)), RED));
    let rim = rim.expect("the left half's outer edge");
    assert!((76..=81).contains(&rim), "16 mm at 0.52 mm a pixel: {rim}");
    let mut looks = all_shown(&set);
    looks[0].opacity = 0.0;
    gpu.set_looks(&looks, None);
    let hidden = drawn(&mut gpu, &axial(&volume, true));
    assert_eq!(
        rgb(&hidden, (rim, ROW)),
        rgb(&plain, (rim, ROW)),
        "no outline round what is hidden"
    );
    assert!(
        near(rgb(&hidden, (LAST_OF_RIGHT, ROW)), BLUE),
        "but the shown half still has its edge"
    );
    gpu.overlay(Overlay::Filled);
    let filled = drawn(&mut gpu, &axial(&volume, true));
    assert_eq!(
        rgb(&filled, IN_THE_LEFT),
        rgb(&plain, IN_THE_LEFT),
        "and no tint"
    );
    // With the right half hidden instead, the line between them is the left half's.
    let mut looks = all_shown(&set);
    looks[1].opacity = 0.0;
    gpu.set_looks(&looks, None);
    gpu.overlay(Overlay::Outline);
    let other = drawn(&mut gpu, &axial(&volume, true));
    assert!(near(rgb(&other, (LAST_OF_RIGHT, ROW)), RED));
}

#[test]
fn a_faded_part_is_tinted_less_and_the_selected_one_is_outlined_in_white() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let (volume, set, plain) = labelled(&mut gpu);
    let mut looks = all_shown(&set);
    looks[0].opacity = 0.5;
    gpu.set_looks(&looks, None);
    gpu.overlay(Overlay::Filled);
    let faded = drawn(&mut gpu, &axial(&volume, true));
    let expected = mixed(rgb(&plain, IN_THE_LEFT), RED, FILL / 2.0);
    assert!(
        near(rgb(&faded, IN_THE_LEFT), expected),
        "{:?}",
        rgb(&faded, IN_THE_LEFT)
    );
    gpu.set_looks(&all_shown(&set), Some(1));
    gpu.overlay(Overlay::Outline);
    let selected = drawn(&mut gpu, &axial(&volume, true));
    let line = rgb(&selected, (LAST_OF_RIGHT, ROW));
    assert!(
        near(line, [255; 3]),
        "the selected half's edge is white: {line:?}"
    );
    gpu.set_looks(&all_shown(&set), Some(99));
    let none = drawn(&mut gpu, &axial(&volume, true));
    assert!(
        near(rgb(&none, (LAST_OF_RIGHT, ROW)), BLUE),
        "a part that is not there selects nothing"
    );
}

#[test]
fn labels_nowhere_near_the_scan_change_nothing() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    let plain = drawn(&mut gpu, &axial(&volume, true));
    let far = ball_labels([0.0, 0.0, 500.0]);
    assert!(gpu.label(Some(&far)));
    gpu.set_looks(&all_shown(&far), None);
    gpu.overlay(Overlay::Filled);
    assert_eq!(drawn(&mut gpu, &axial(&volume, true)), plain);
}

#[test]
fn labels_keep_their_place_over_a_scan_that_is_tilted() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let (sin, cos) = 20.0_f64.to_radians().sin_cos();
    let steps = [[1.0, 0.0, 0.0], [0.0, cos, sin], [0.0, -sin, cos]];
    let tilted = ball_on([48; 3], centred([48; 3], steps));
    gpu.upload(&tilted).expect("uploads");
    let set = ball_labels([0.0; 3]);
    assert!(gpu.label(Some(&set)));
    gpu.set_looks(&all_shown(&set), None);
    gpu.overlay(Overlay::Outline);
    let outlined = drawn(&mut gpu, &axial(&tilted, true));
    let line = rgb(&outlined, (LAST_OF_RIGHT, ROW));
    assert!(
        near(line, BLUE),
        "the line between the halves is still down the middle: {line:?}"
    );
}
