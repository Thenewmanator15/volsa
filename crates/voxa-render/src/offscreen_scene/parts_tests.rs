//! Labelled parts drawn as solids: each in its colour, on the side of the picture the
//! patient has it, with what is hidden gone and what is faded seen through.

use voxa_labels::LabelSet;

use super::*;
use crate::camera::NamedView;
use crate::labels::PartLook;
use crate::scene::{CutPlane, Look, SceneParams};
use crate::testing::{
    BLUE, GREEN, RED, SIDE, ball, ball_labels, headless_scene, scene_of, silhouette,
};

/// The middle of a test picture.
const MIDDLE: i32 = 48;

fn seen_from(view: NamedView) -> SceneParams {
    scene_of(&ball(), view, Look::Parts)
}

fn drawn(gpu: &mut HeadlessScene, scene: &SceneParams) -> Vec<u8> {
    gpu.render_parts(scene, [SIDE, SIDE]).expect("renders")
}

/// The pixel this many millimetres to the right of, and below, the middle of the picture.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a few tens of pixels either side of the middle"
)]
fn pixel(scene: &SceneParams, right_mm: f64, down_mm: f64) -> (u32, u32) {
    let per_mm = 1.0 / scene.camera.mm_per_px;
    let at = |mm: f64| {
        let from_middle = (mm * per_mm).round() as i32;
        u32::try_from(MIDDLE + from_middle).expect("in the picture")
    };
    (at(right_mm), at(down_mm))
}

fn rgb(rgba: &[u8], (x, y): (u32, u32)) -> [u8; 3] {
    let at = ((y * SIDE + x) * 4) as usize;
    [rgba[at], rgba[at + 1], rgba[at + 2]]
}

/// Whether a pixel is plainly of one hue: that channel twice the other two, and not dark.
fn is(colour: [u8; 3], of: [u8; 3]) -> bool {
    let strongest = (0..3).max_by_key(|at| of[*at]).expect("three channels");
    let others = (0..3).filter(|at| *at != strongest);
    colour[strongest] > 40
        && others
            .clone()
            .all(|at| u16::from(colour[strongest]) > 2 * u16::from(colour[at]))
}

fn count(rgba: &[u8], of: [u8; 3]) -> usize {
    rgba.chunks(4)
        .filter(|pixel| is([pixel[0], pixel[1], pixel[2]], of))
        .count()
}

fn all_shown(set: &LabelSet) -> Vec<PartLook> {
    let solid = |part: &voxa_labels::Part| PartLook {
        colour: part.colour,
        opacity: 1.0,
    };
    set.parts.iter().map(solid).collect()
}

/// A scene with the ball's labels on the device, every part shown.
fn labelled() -> Option<(HeadlessScene, LabelSet)> {
    let mut gpu = headless_scene()?;
    let set = ball_labels([0.0; 3]);
    assert!(gpu.label(Some(&set)), "there is room for them");
    Some((gpu, set))
}

#[test]
fn each_half_is_on_the_side_of_the_picture_the_patient_has_it() {
    let Some((mut gpu, _)) = labelled() else {
        return;
    };
    // From the front the patient's left is on the right of the picture.
    let front = seen_from(NamedView::Anterior);
    let rgba = drawn(&mut gpu, &front);
    let (left_half, right_half) = (pixel(&front, 4.0, 0.0), pixel(&front, -4.0, 0.0));
    assert!(
        is(rgb(&rgba, left_half), RED),
        "{:?}",
        rgb(&rgba, left_half)
    );
    assert!(
        is(rgb(&rgba, right_half), BLUE),
        "{:?}",
        rgb(&rgba, right_half)
    );
    let on_the_bead = pixel(&front, 12.0, 0.0);
    assert!(
        is(rgb(&rgba, on_the_bead), GREEN),
        "the box about the bead: {:?}",
        rgb(&rgba, on_the_bead)
    );
    // From behind they change sides.
    let back = seen_from(NamedView::Posterior);
    let rgba = drawn(&mut gpu, &back);
    assert!(is(rgb(&rgba, pixel(&back, 4.0, 0.0)), BLUE));
    assert!(is(rgb(&rgba, pixel(&back, -4.0, 0.0)), RED));
    // From the patient's left only the left half is seen, and from the right only the right.
    let rgba = drawn(&mut gpu, &seen_from(NamedView::Left));
    assert!(
        count(&rgba, RED) > 500 && count(&rgba, BLUE) == 0,
        "from the left"
    );
    let rgba = drawn(&mut gpu, &seen_from(NamedView::Right));
    assert!(
        count(&rgba, BLUE) > 500 && count(&rgba, RED) == 0,
        "from the right"
    );
}

#[test]
fn every_pixel_is_opaque_and_there_is_black_round_the_solids() {
    let Some((mut gpu, _)) = labelled() else {
        return;
    };
    let front = seen_from(NamedView::Anterior);
    let rgba = drawn(&mut gpu, &front);
    assert!(rgba.chunks(4).all(|pixel| pixel[3] == 255));
    assert_eq!(rgb(&rgba, (1, 1)), [0, 0, 0]);
    assert_eq!(rgb(&rgba, (94, 94)), [0, 0, 0]);
    // The solids are as wide as the ball the Surface look draws: 32 mm.
    let [wide, high] = silhouette(&rgba, 8);
    let (across, _) = pixel(&front, 32.0, 0.0);
    let across = across - 48;
    assert!(
        wide.abs_diff(across) <= 2 && high.abs_diff(across) <= 2,
        "{wide} x {high} for {across}"
    );
}

#[test]
fn a_hidden_part_uncovers_what_is_behind_it_and_a_faded_one_lets_it_show() {
    let Some((mut gpu, set)) = labelled() else {
        return;
    };
    let side = seen_from(NamedView::Left);
    let middle = pixel(&side, 0.0, 6.0);
    let whole = drawn(&mut gpu, &side);
    assert!(is(rgb(&whole, middle), RED));
    let mut looks = all_shown(&set);
    looks[0].opacity = 0.0;
    gpu.set_looks(&looks, None);
    let uncovered = drawn(&mut gpu, &side);
    assert!(
        is(rgb(&uncovered, middle), BLUE),
        "the flat face of the other half: {:?}",
        rgb(&uncovered, middle)
    );
    assert_eq!(count(&uncovered, RED), 0, "none of what is hidden is drawn");
    looks[0].opacity = 0.5;
    gpu.set_looks(&looks, None);
    let through = drawn(&mut gpu, &side);
    let [red, _, blue] = rgb(&through, middle);
    assert!(red > 40 && blue > 40, "both show: red {red}, blue {blue}");
    let [solid_red, ..] = rgb(&whole, middle);
    assert!(
        red < solid_red,
        "and the faded half is fainter than it was: {red} against {solid_red}"
    );
}

#[test]
fn a_cut_shows_the_parts_colours_flat_on_its_face() {
    let Some((mut gpu, _)) = labelled() else {
        return;
    };
    let mut front = seen_from(NamedView::Anterior);
    front.cuts = vec![CutPlane::facing(&front.camera)];
    let rgba = drawn(&mut gpu, &front);
    let near = |a: [u8; 3], b: [u8; 3]| a.into_iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 2);
    let (left, right) = (
        rgb(&rgba, pixel(&front, 6.0, 3.0)),
        rgb(&rgba, pixel(&front, -6.0, 3.0)),
    );
    assert!(
        near(left, RED) && near(right, BLUE),
        "the face is unlit: {left:?}, {right:?}"
    );
    assert_eq!(
        count(&rgba, GREEN),
        0,
        "the bead's box was in the half cut away"
    );
}

#[test]
fn the_selected_part_has_a_pale_rim_and_the_others_are_as_they_were() {
    let Some((mut gpu, set)) = labelled() else {
        return;
    };
    let front = seen_from(NamedView::Anterior);
    let plain = drawn(&mut gpu, &front);
    gpu.set_looks(&all_shown(&set), Some(1));
    let selected = drawn(&mut gpu, &front);
    // Near the edge of the blue half, where its surface turns away from the viewer.
    let rim = pixel(&front, -14.0, 3.0);
    let (before, after) = (rgb(&plain, rim), rgb(&selected, rim));
    let sum = |colour: [u8; 3]| colour.iter().map(|part| u32::from(*part)).sum::<u32>();
    assert!(
        sum(after) > sum(before) + 30,
        "paler at the rim: {before:?} to {after:?}"
    );
    let other = pixel(&front, 6.0, 3.0);
    assert_eq!(
        rgb(&selected, other),
        rgb(&plain, other),
        "the red half is untouched"
    );
}

#[test]
fn the_selected_part_is_paler_on_a_cuts_face_and_the_other_is_as_it_was() {
    let Some((mut gpu, set)) = labelled() else {
        return;
    };
    let mut front = seen_from(NamedView::Anterior);
    front.cuts = vec![CutPlane::facing(&front.camera)];
    let plain = drawn(&mut gpu, &front);
    gpu.set_looks(&all_shown(&set), Some(1));
    let selected = drawn(&mut gpu, &front);
    // On the cut's face: the red half on the picture's right, the blue, selected, on its left.
    let (red, blue) = (pixel(&front, 6.0, 3.0), pixel(&front, -6.0, 3.0));
    assert_eq!(
        rgb(&selected, red),
        rgb(&plain, red),
        "not the one selected"
    );
    let (before, after) = (rgb(&plain, blue), rgb(&selected, blue));
    let sum = |colour: [u8; 3]| colour.iter().map(|part| u32::from(*part)).sum::<u32>();
    assert!(
        sum(after) > sum(before) + 60,
        "paler on the face: {before:?} to {after:?}"
    );
    assert!(after[2] > after[0], "and still its own colour: {after:?}");
}

#[test]
fn the_blocks_with_nothing_labelled_in_them_are_found_on_the_device() {
    let Some((gpu, set)) = labelled() else {
        return;
    };
    let held = gpu.part_blocks_as_held().expect("labels are held");
    // The same on the processor: whether any voxel of each block of 8 a side holds a part.
    let mut filled = vec![false; 4 * 4 * 4];
    for (at, number) in set.numbers.iter().enumerate() {
        let (col, row, slice) = (at % 30, at / 30 % 30, at / 900);
        filled[(slice / 8 * 4 + row / 8) * 4 + col / 8] |= *number != 0;
    }
    let found: Vec<bool> = held.iter().map(|bits| *bits != 0).collect();
    assert_eq!(found, filled);
    let empty = filled.iter().filter(|filled| !**filled).count();
    assert!(
        empty > 20 && empty < 60,
        "some of each: {empty} empty of 64"
    );
}

#[test]
fn stepping_over_the_empty_blocks_draws_the_very_same_picture() {
    let Some((mut gpu, set)) = labelled() else {
        return;
    };
    let mut faded = all_shown(&set);
    faded[0].opacity = 0.4;
    let turned = |view| {
        let mut scene = seen_from(view);
        scene.camera = scene.camera.orbited([37.0, 21.0]);
        scene
    };
    let mut cut = turned(NamedView::Left);
    cut.cuts = vec![CutPlane::facing(&seen_from(NamedView::Anterior).camera)];
    let scenes = [
        seen_from(NamedView::Anterior),
        seen_from(NamedView::Superior),
        turned(NamedView::Posterior),
        cut,
    ];
    for looks in [all_shown(&set), faded] {
        gpu.set_looks(&looks, Some(1));
        for scene in &scenes {
            let stepped_over = SceneParams {
                skip_empty: true,
                ..scene.clone()
            };
            let walked = SceneParams {
                skip_empty: false,
                ..scene.clone()
            };
            let (quick, slow) = (drawn(&mut gpu, &stepped_over), drawn(&mut gpu, &walked));
            assert!(quick == slow, "pixel for pixel");
            assert!(count(&quick, BLUE) + count(&quick, RED) > 0 || !quick.is_empty());
        }
    }
}

#[test]
fn half_quality_is_nearly_the_same_picture_and_no_labels_is_a_black_one() {
    let Some((mut gpu, _)) = labelled() else {
        return;
    };
    let front = seen_from(NamedView::Anterior);
    let full = drawn(&mut gpu, &front);
    let coarse = SceneParams {
        quality: 0.5,
        ..front.clone()
    };
    let rough = drawn(&mut gpu, &coarse);
    let apart: u64 = full
        .iter()
        .zip(&rough)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    let mean = apart as f64 / full.len() as f64;
    assert!(mean < 4.0, "{mean} grey levels apart on average");
    assert!(gpu.has_parts());
    gpu.label(None);
    assert!(!gpu.has_parts());
    let nothing = drawn(&mut gpu, &front);
    assert!(nothing.chunks(4).all(|pixel| pixel == [0, 0, 0, 255]));
}

#[test]
fn the_volume_pipeline_draws_the_parts_look_as_a_surface() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    let surface = scene_of(&volume, NamedView::Anterior, Look::Surface);
    let parts = scene_of(&volume, NamedView::Anterior, Look::Parts);
    let as_surface = gpu.render(&surface, [SIDE, SIDE]).expect("renders");
    assert_eq!(
        gpu.render(&parts, [SIDE, SIDE]).expect("renders"),
        as_surface
    );
}
