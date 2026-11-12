//! The see-through look and the air: what is fainter than a tenth of the window is not
//! drawn, so the faint noise a scanner leaves in the air neither shows nor dims the patient.

use volsa_core::Volume;

use super::tests::{MIDDLE, at, drawn};
use super::*;
use crate::camera::NamedView;
use crate::scene::Look;
use crate::testing::{MM, brightest, centred, headless_scene, mean_difference, scene_of};

/// Columns, rows and slices of the hazy volume, 1 mm apart: 96 mm deep as seen from the front.
const DIMS: [u16; 3] = [48, 96, 48];
/// A value 8% of the way up the ball's window: fainter than tissue is taken to be.
const HAZE: f32 = 32.0;

/// A volume all of one faint value, with a plate of the brightest value 16 mm thick across
/// its middle if `plate`: 40 mm of haze lie in front of the plate from the front and the back.
fn hazy(haze: f32, plate: bool) -> Volume {
    let [cols, rows, slices] = DIMS.map(usize::from);
    let value = |at: usize| {
        let row = at / cols % rows;
        if plate && (40..56).contains(&row) {
            400.0
        } else {
            haze
        }
    };
    let values = (0..cols * rows * slices).map(value).collect();
    Volume::new(DIMS.map(u32::from), values, centred(DIMS, MM)).expect("adds up")
}

fn seen(gpu: &mut HeadlessScene, volume: &Volume, view: NamedView, skip_empty: bool) -> Vec<u8> {
    gpu.upload(volume).expect("uploads");
    let scene = SceneParams {
        skip_empty,
        ..scene_of(volume, view, Look::Vr)
    };
    drawn(gpu, &scene)
}

#[test]
fn haze_fainter_than_a_tenth_of_the_window_is_not_drawn() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let rgba = seen(&mut gpu, &hazy(HAZE, false), NamedView::Anterior, false);
    assert_eq!(brightest(&rgba), 0, "96 mm of haze and nothing else");
    // Just above a tenth of the window it is tissue, faint as it is.
    let rgba = seen(&mut gpu, &hazy(60.0, false), NamedView::Anterior, false);
    assert!(
        brightest(&rgba) > 0,
        "15% of the way up the window is drawn"
    );
}

#[test]
fn haze_does_not_dim_what_is_behind_it() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    for view in [NamedView::Anterior, NamedView::Posterior] {
        let alone = at(&seen(&mut gpu, &hazy(0.0, true), view, false), MIDDLE);
        let in_haze = at(&seen(&mut gpu, &hazy(HAZE, true), view, false), MIDDLE);
        assert!(alone > 100, "the plate stops half the light: {alone}");
        assert!(
            alone.abs_diff(in_haze) <= 2,
            "{view:?}: {alone} alone, {in_haze} behind 40 mm of haze"
        );
    }
}

#[test]
fn stepping_over_the_haze_gives_the_same_picture() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = hazy(HAZE, true);
    let every_step = seen(&mut gpu, &volume, NamedView::Left, false);
    let skipping = seen(&mut gpu, &volume, NamedView::Left, true);
    assert!(brightest(&every_step) > 100, "the plate is seen edge on");
    let apart = mean_difference(&every_step, &skipping);
    assert!(apart < 0.01, "they differ by {apart} grey levels a pixel");
}
