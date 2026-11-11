//! Stepping over the empty parts of a volume must not change the picture.

use voxa_core::WindowLevel;

use super::tests::drawn;
use super::*;
use crate::camera::NamedView;
use crate::scene::Look;
use crate::testing::{MM, SIDE, ball, ball_on, brightest, centred, grey, headless_scene, scene_of};
use crate::volume_texture::{BLOCK, Placed};

/// The most any one pixel differs between two test pictures, in grey levels.
fn most_apart(one: &[u8], other: &[u8]) -> u8 {
    let pixels = (0..SIDE).flat_map(|y| (0..SIDE).map(move |x| (x, y)));
    pixels
        .map(|(x, y)| grey(one, x, y).abs_diff(grey(other, x, y)))
        .max()
        .unwrap_or(0)
}

/// How far apart the pictures of one scene are with empty space skipped and without, having
/// checked that the scene draws something at all. `None` with no adapter.
///
/// The ball is in the middle of 96 voxels each way, so that it has two blocks and more of
/// empty space on every side: one that touches its own blocks, and at least one that can be
/// stepped over.
fn skipping_changes(look: Look, change: impl Fn(&mut SceneParams)) -> Option<u8> {
    let mut gpu = headless_scene()?;
    let volume = ball_on([96; 3], centred([96; 3], MM));
    gpu.upload(&volume).expect("uploads");
    let mut plain = scene_of(&volume, NamedView::Anterior, look);
    change(&mut plain);
    let skipped = SceneParams {
        skip_empty: true,
        ..plain.clone()
    };
    let (plain, skipped) = (drawn(&mut gpu, &plain), drawn(&mut gpu, &skipped));
    assert!(brightest(&plain) > 0, "something is drawn");
    Some(most_apart(&plain, &skipped))
}

#[test]
fn skipping_empty_space_leaves_a_picture_from_the_front_as_it_was() {
    for look in [Look::Surface, Look::Vr] {
        let Some(apart) = skipping_changes(look, |_| {}) else {
            return;
        };
        assert!(apart <= 2, "{look:?}: a pixel is {apart} grey levels out");
    }
}

#[test]
fn skipping_empty_space_leaves_a_picture_from_an_angle_as_it_was() {
    for look in [Look::Surface, Look::Vr] {
        let turn = |scene: &mut SceneParams| scene.camera = scene.camera.orbited([40.0, 25.0]);
        let Some(apart) = skipping_changes(look, turn) else {
            return;
        };
        assert!(apart <= 2, "{look:?}: a pixel is {apart} grey levels out");
    }
}

#[test]
fn skipping_follows_the_window_as_it_changes() {
    // With the window's low end above the shell, the shell's blocks are empty too.
    for look in [Look::Surface, Look::Vr] {
        let narrow = |scene: &mut SceneParams| {
            scene.window = WindowLevel::new(300.0, 200.0);
            scene.camera = scene.camera.orbited([-70.0, -35.0]);
        };
        let Some(apart) = skipping_changes(look, narrow) else {
            return;
        };
        assert!(apart <= 2, "{look:?}: a pixel is {apart} grey levels out");
    }
}

#[test]
fn skipping_treats_a_value_at_the_very_bottom_of_the_window_as_empty() {
    // The shell is exactly at the low end of this window: it is not drawn, skipped or not,
    // and the core and bead above it are drawn just the same.
    let at_the_shell = |scene: &mut SceneParams| scene.window = WindowLevel::new(250.0, 300.0);
    let Some(apart) = skipping_changes(Look::Surface, at_the_shell) else {
        return;
    };
    assert!(apart <= 2, "a pixel is {apart} grey levels out");
}

#[test]
fn skipping_is_exact_with_cuts_and_at_low_quality() {
    use crate::scene::CutPlane;
    use voxa_core::Point;
    let cut_and_coarse = |scene: &mut SceneParams| {
        scene.cuts = vec![CutPlane {
            point: Point::new(3.0, -2.0, 1.0),
            normal: [0.3, -1.0, 0.2],
        }];
        scene.quality = 0.4;
        scene.camera = scene.camera.orbited([25.0, 60.0]);
    };
    for look in [Look::Surface, Look::Vr] {
        let Some(apart) = skipping_changes(look, cut_and_coarse) else {
            return;
        };
        assert!(apart <= 2, "{look:?}: a pixel is {apart} grey levels out");
    }
}

#[test]
fn skipping_leaves_the_brightest_value_look_as_it_was() {
    // What is skipped is no brighter than the bottom of the window, which is black anyway.
    for window in [
        WindowLevel::new(200.0, 400.0),
        WindowLevel::new(300.0, 200.0),
    ] {
        let turned = |scene: &mut SceneParams| {
            scene.window = window;
            scene.camera = scene.camera.orbited([40.0, 25.0]);
        };
        let Some(apart) = skipping_changes(Look::Mip, turned) else {
            return;
        };
        assert!(apart <= 2, "a pixel is {apart} grey levels out");
    }
}

#[test]
fn skipping_steps_over_empty_blocks_in_the_brightest_value_look_too() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let plain = SceneParams {
        look: Look::Mip,
        ..ball_marked(&mut gpu, 100.0, 100.0)
    };
    let skipped = SceneParams {
        skip_empty: true,
        ..plain.clone()
    };
    let (plain, skipped) = (
        total(&drawn(&mut gpu, &plain)),
        total(&drawn(&mut gpu, &skipped)),
    );
    // Every block is marked as holding nothing above the bottom of the window.
    assert!(plain > 0 && skipped == 0, "{skipped} skipping, {plain} not");
}

/// The summed grey of a test picture.
fn total(rgba: &[u8]) -> u64 {
    let pixels = (0..SIDE).flat_map(|y| (0..SIDE).map(move |x| (x, y)));
    pixels.map(|(x, y)| u64::from(grey(rgba, x, y))).sum()
}

/// The ball put on the device with every block marked, whatever it holds, as holding
/// nothing above `largest`; and a scene of it in the see-through look through a window 300
/// wide whose low end is `low`, so that tissue begins 30 above `low`.
fn ball_marked(gpu: &mut HeadlessScene, largest: f32, low: f32) -> SceneParams {
    let volume = ball();
    let blocks = volume.dims().map(|count| count.div_ceil(BLOCK));
    let marks = vec![largest; blocks.iter().product::<u32>() as usize];
    let (device, queue) = (&gpu.gpu.device, &gpu.gpu.queue);
    let placed = Placed::with_maxima(device, queue, &volume, (&marks, blocks)).expect("room");
    gpu.pipeline.show(device, placed);
    let window = WindowLevel::new(low + 150.0, 300.0);
    SceneParams {
        window,
        ..scene_of(&volume, NamedView::Anterior, Look::Vr)
    }
}

#[test]
fn skipping_steps_over_a_block_whose_largest_value_is_just_short_of_where_tissue_begins() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let plain = ball_marked(&mut gpu, 100.0, 71.0);
    let skipped = SceneParams {
        skip_empty: true,
        ..plain.clone()
    };
    let (plain, skipped) = (
        total(&drawn(&mut gpu, &plain)),
        total(&drawn(&mut gpu, &skipped)),
    );
    // Every block is taken as empty, so no line of sight meets anything.
    assert!(plain > 0 && skipped == 0, "{skipped} skipping, {plain} not");
}

#[test]
fn skipping_keeps_a_block_whose_largest_value_is_just_past_where_tissue_begins() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let plain = ball_marked(&mut gpu, 100.0, 69.0);
    let skipped = SceneParams {
        skip_empty: true,
        ..plain.clone()
    };
    let (plain, skipped) = (drawn(&mut gpu, &plain), drawn(&mut gpu, &skipped));
    assert!(
        total(&plain) > 0 && most_apart(&plain, &skipped) <= 2,
        "nothing is stepped over"
    );
}
