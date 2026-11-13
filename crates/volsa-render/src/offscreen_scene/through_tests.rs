//! A part drawn through the rest: seen in 3D behind the other solids, fainter there than in
//! front of them, and gone again when it is hidden.

use super::*;
use crate::camera::NamedView;
use crate::labels::PartLook;
use crate::scene::{Look, SceneParams};
use crate::testing::{SIDE, ball, ball_labels, headless_scene, scene_of};

/// A pixel a little below the middle of a test picture, inside both halves of the ball.
fn middle(rgba: &[u8]) -> [u8; 3] {
    let at = ((52 * SIDE + 48) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2]]
}

fn drawn(gpu: &mut HeadlessScene, view: NamedView) -> Vec<u8> {
    let scene: SceneParams = scene_of(&ball(), view, Look::Parts);
    gpu.render_parts(&scene, [SIDE, SIDE]).expect("renders")
}

/// Every part of the ball's labels shown, the right half (blue) drawn through the rest, as
/// solid as `opacity`.
fn looks(opacity: f32) -> Vec<PartLook> {
    let set = ball_labels([0.0; 3]);
    let look = |(at, part): (usize, &volsa_labels::Part)| PartLook {
        colour: part.colour,
        opacity: if at == 1 { opacity } else { 1.0 },
        through: at == 1,
    };
    set.parts.iter().enumerate().map(look).collect()
}

#[test]
fn a_part_drawn_through_the_rest_is_seen_behind_them_fainter_than_in_front() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    assert!(gpu.label(Some(&ball_labels([0.0; 3]))), "there is room");
    // From the patient's left the left half, red, hides the right half, blue.
    let [_, _, hidden_blue] = middle(&drawn(&mut gpu, NamedView::Left));
    gpu.set_looks(&looks(1.0), None);
    let [red, _, blue] = middle(&drawn(&mut gpu, NamedView::Left));
    assert!(
        blue > red && blue > hidden_blue + 60,
        "seen through the red half: red {red}, blue {blue}, before {hidden_blue}"
    );
    // From the right it is in front, and stronger.
    let [front_red, _, front_blue] = middle(&drawn(&mut gpu, NamedView::Right));
    assert!(
        front_blue > blue && u16::from(front_blue) > 2 * u16::from(front_red),
        "in front: red {front_red}, blue {front_blue}; behind, blue {blue}"
    );
    // Hidden, it is not drawn at all, and the red half is as it was.
    gpu.set_looks(&looks(0.0), None);
    let [red, _, blue] = middle(&drawn(&mut gpu, NamedView::Left));
    assert!(blue < red && red > 60, "hidden: red {red}, blue {blue}");
}

#[test]
fn a_fainter_part_drawn_through_shows_less_strongly() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    assert!(gpu.label(Some(&ball_labels([0.0; 3]))), "there is room");
    gpu.set_looks(&looks(1.0), None);
    let [_, _, strong] = middle(&drawn(&mut gpu, NamedView::Left));
    gpu.set_looks(&looks(0.3), None);
    let [_, _, faint] = middle(&drawn(&mut gpu, NamedView::Left));
    assert!(faint + 20 < strong, "faint {faint} against strong {strong}");
}
