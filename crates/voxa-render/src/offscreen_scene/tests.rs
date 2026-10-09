//! Pictures of a volume drawn on a graphics device with no window, and measured.

use voxa_core::WindowLevel;

use super::*;
use crate::camera::NamedView;
use crate::scene::Look;
use crate::testing::{
    FAINT, HALF_SHELL, MM, SIDE, ball, ball_on, bead_middle, brightest, centred, grey, halves,
    headless_scene, lean, mean_difference, scene_of, silhouette,
};

/// The grey of the shell, the core and the bead through the ball's window.
pub(super) const SHELL: u8 = 64;
pub(super) const CORE: u8 = 191;
const BEAD: u8 = 255;
/// A pixel 14 mm to the left of the middle of a fitted picture: in the shell, clear of the core.
pub(super) const IN_THE_SHELL: (u32, u32) = (23, 48);
pub(super) const MIDDLE: (u32, u32) = (48, 48);

pub(super) fn drawn(gpu: &mut HeadlessScene, params: &SceneParams) -> Vec<u8> {
    gpu.render(params, [SIDE, SIDE]).expect("renders")
}

/// The test ball, uploaded and drawn from one side in one look. `None` with no adapter.
pub(super) fn ball_from(view: NamedView, look: Look) -> Option<Vec<u8>> {
    let mut gpu = headless_scene()?;
    let volume = ball();
    assert_eq!(gpu.upload(&volume).expect("uploads"), 0, "it fits as it is");
    Some(drawn(&mut gpu, &scene_of(&volume, view, look)))
}

pub(super) fn at(rgba: &[u8], (x, y): (u32, u32)) -> u8 {
    grey(rgba, x, y)
}

#[test]
fn mip_shows_the_brightest_value_on_each_line_of_sight() {
    let Some(rgba) = ball_from(NamedView::Anterior, Look::Mip) else {
        return;
    };
    assert_eq!(rgba.len(), (SIDE * SIDE * 4) as usize);
    assert_eq!(
        rgba[..4],
        [0, 0, 0, 255],
        "outside the volume is opaque black"
    );
    assert!(
        at(&rgba, MIDDLE).abs_diff(CORE) <= 3,
        "the core: {}",
        at(&rgba, MIDDLE)
    );
    let shell = at(&rgba, IN_THE_SHELL);
    assert!(shell.abs_diff(SHELL) <= 3, "the shell: {shell}");
    assert_eq!(brightest(&rgba), BEAD, "the bead");
    let [across, _] = bead_middle(&rgba);
    assert!(
        across > f64::from(SIDE / 2),
        "the patient's left is on the right: {across}"
    );
}

#[test]
fn the_bead_is_where_the_patient_has_it_from_every_side() {
    // The bead is 12 mm to the patient's left and 14 mm to the front, level with the middle.
    // A fitted picture has 48 mm across 86.4 pixels, so those are 21.6 and 25.2 pixels.
    let expected = [
        (NamedView::Anterior, [48.0 + 21.6, 48.0]),
        (NamedView::Posterior, [48.0 - 21.6, 48.0]),
        (NamedView::Left, [48.0 - 25.2, 48.0]),
        (NamedView::Right, [48.0 + 25.2, 48.0]),
        (NamedView::Superior, [48.0 - 21.6, 48.0 - 25.2]),
        (NamedView::Inferior, [48.0 + 21.6, 48.0 - 25.2]),
    ];
    for (view, [across, down]) in expected {
        let Some(rgba) = ball_from(view, Look::Mip) else {
            return;
        };
        let [found_across, found_down] = bead_middle(&rgba);
        let off = (found_across - across).abs().max((found_down - down).abs());
        assert!(off <= 1.5, "{view:?}: at {found_across}, {found_down}");
    }
}

#[test]
fn vr_is_brighter_through_the_core_and_on_the_side_of_the_bead() {
    let Some(rgba) = ball_from(NamedView::Anterior, Look::Vr) else {
        return;
    };
    assert_eq!(
        rgba[..4],
        [0, 0, 0, 255],
        "outside the volume is opaque black"
    );
    let (middle, shell) = (at(&rgba, MIDDLE), at(&rgba, IN_THE_SHELL));
    assert!(shell > 0, "the shell can be seen");
    assert!(
        middle > shell,
        "{middle} through the core, {shell} through the shell alone"
    );
    let [left, right] = halves(&rgba);
    assert!(right > left, "the bead is on the right: {left} and {right}");
}

#[test]
fn vr_from_behind_has_the_bead_on_the_other_side() {
    let Some(rgba) = ball_from(NamedView::Posterior, Look::Vr) else {
        return;
    };
    let [left, right] = halves(&rgba);
    assert!(left > right, "{left} and {right}");
}

#[test]
fn every_pixel_of_every_look_is_an_opaque_grey() {
    for look in [Look::Surface, Look::Vr, Look::Mip] {
        let Some(rgba) = ball_from(NamedView::Left, look) else {
            return;
        };
        let grey = |px: &[u8; 4]| px[0] == px[1] && px[1] == px[2] && px[3] == 255;
        assert!(rgba.as_chunks::<4>().0.iter().all(grey), "{look:?}");
    }
}

#[test]
fn thick_slices_are_drawn_in_their_true_proportions() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let grid = [48, 48, 16];
    let thick = ball_on(
        grid,
        centred(grid, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 3.0]]),
    );
    gpu.upload(&thick).expect("uploads");
    let rgba = drawn(&mut gpu, &scene_of(&thick, NamedView::Anterior, Look::Mip));
    // Everything drawn on is counted: between slices 3 mm apart the edge itself is a blur.
    let [wide, high] = silhouette(&rgba, FAINT);
    assert!(
        wide.abs_diff(high) <= 2,
        "a ball should be round: {wide} by {high}"
    );
    assert!(wide.abs_diff(59) <= 3, "and 33 mm across: {wide}");
}

#[test]
fn a_tilted_stack_is_drawn_in_its_true_proportions() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    // Each slice is 3 mm above the last and 0.6 mm further back.
    let grid = [48, 48, 16];
    let tilted = ball_on(
        grid,
        centred(grid, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.6, 3.0]]),
    );
    gpu.upload(&tilted).expect("uploads");
    let front = drawn(&mut gpu, &scene_of(&tilted, NamedView::Anterior, Look::Mip));
    let [wide, high] = silhouette(&front, FAINT);
    assert!(
        wide.abs_diff(high) <= 2,
        "a ball should be round: {wide} by {high}"
    );
    // From the side the tilt is across the picture. Drawn as if the slices were stacked
    // straight, the top of the ball would lie some 7 pixels in front of the bottom.
    let side = drawn(&mut gpu, &scene_of(&tilted, NamedView::Left, Look::Mip));
    assert!(
        lean(&side).abs() < 1.0,
        "the ball leans by {} pixels",
        lean(&side)
    );

    // The measure can tell: the same values placed as a straight stack do lean.
    let straight = centred(grid, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 3.0]]);
    let skewed = Volume::new(tilted.dims(), tilted.values().to_vec(), straight).expect("adds up");
    gpu.upload(&skewed).expect("uploads");
    let side = drawn(&mut gpu, &scene_of(&skewed, NamedView::Left, Look::Mip));
    assert!(
        lean(&side).abs() > 3.0,
        "a skewed ball leans by only {}",
        lean(&side)
    );
}

#[test]
fn half_quality_gives_nearly_the_same_picture() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    let full = scene_of(&volume, NamedView::Anterior, Look::Vr);
    let coarse = SceneParams {
        quality: 0.5,
        ..full.clone()
    };
    let apart = mean_difference(&drawn(&mut gpu, &full), &drawn(&mut gpu, &coarse));
    assert!(apart < 4.0, "{apart} grey levels apart on average");
}

#[test]
fn a_volume_too_large_for_the_device_is_reduced_and_drawn_in_the_same_place() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let grid = [64; 3];
    let fine = ball_on(
        grid,
        centred(grid, MM.map(|step| step.map(|part| part * 0.75))),
    );
    let scene = scene_of(&fine, NamedView::Anterior, Look::Mip);
    assert_eq!(gpu.upload(&fine).expect("uploads"), 0);
    let whole = drawn(&mut gpu, &scene);

    let within = |gpu: &mut HeadlessScene, max_side, max_bytes| {
        let (device, queue) = (&gpu.gpu.device, &gpu.gpu.queue);
        gpu.pipeline
            .upload_within(device, queue, &fine, max_side, max_bytes)
            .expect("uploads")
    };
    assert_eq!(
        within(&mut gpu, 32, u64::MAX),
        1,
        "halved once to fit the side"
    );
    let reduced = drawn(&mut gpu, &scene);
    let [wide, high] = silhouette(&reduced, HALF_SHELL);
    let [was_wide, was_high] = silhouette(&whole, HALF_SHELL);
    assert!(
        wide.abs_diff(was_wide) <= 3 && high.abs_diff(was_high) <= 3,
        "{wide} by {high}"
    );
    assert!(
        at(&reduced, MIDDLE).abs_diff(CORE) <= 3,
        "the core is still the core"
    );

    assert_eq!(
        within(&mut gpu, u32::MAX, 64 * 64 * 64 * 2 - 1),
        1,
        "and to fit the bytes"
    );
    assert_eq!(within(&mut gpu, 0, 0), 6, "down to one voxel if it must be");
    drawn(&mut gpu, &scene);
}

#[test]
fn nothing_uploaded_draws_black() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    assert!(!gpu.pipeline.has_volume());
    let rgba = drawn(&mut gpu, &scene_of(&ball(), NamedView::Anterior, Look::Mip));
    assert!(
        rgba.as_chunks::<4>()
            .0
            .iter()
            .all(|px| *px == [0, 0, 0, 255])
    );
    gpu.upload(&ball()).expect("uploads");
    assert!(gpu.pipeline.has_volume());
}

#[test]
fn a_changed_window_changes_the_picture_with_no_new_upload() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    let wide = scene_of(&volume, NamedView::Anterior, Look::Mip);
    let narrow = SceneParams {
        window: WindowLevel::new(300.0, 200.0),
        ..wide.clone()
    };
    let through_wide = drawn(&mut gpu, &wide);
    let through_narrow = drawn(&mut gpu, &narrow);
    assert_eq!(
        at(&through_narrow, IN_THE_SHELL),
        0,
        "the shell is below the window"
    );
    let core = at(&through_narrow, MIDDLE);
    assert!(
        core.abs_diff(128) <= 3,
        "the core is in the middle of it: {core}"
    );
    assert_eq!(drawn(&mut gpu, &wide), through_wide, "and back again");
}

#[test]
fn a_second_volume_replaces_the_first() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    let empty = Volume::new([2, 2, 2], vec![0.0; 8], centred([2; 3], MM)).expect("adds up");
    gpu.upload(&empty).expect("uploads");
    let rgba = drawn(&mut gpu, &scene_of(&volume, NamedView::Anterior, Look::Mip));
    assert_eq!(brightest(&rgba), 0, "nothing of the ball is left");
}

#[test]
fn a_picture_of_no_size_is_refused() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let scene = scene_of(&ball(), NamedView::Anterior, Look::Mip);
    assert!(matches!(
        gpu.render(&scene, [0, 96]),
        Err(RenderError::EmptyTarget)
    ));
}
