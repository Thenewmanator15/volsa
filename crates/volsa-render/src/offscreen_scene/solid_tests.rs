//! Pictures in the Surface look: a solid, lit surface that hides what is behind it.

use super::tests::{MIDDLE, SHELL, at, ball_from, drawn};
use super::*;
use crate::camera::NamedView;
use crate::scene::Look;
use crate::testing::{ball, brightest, headless_scene, mean_difference, scene_of};

/// Pixels of a fitted picture of the ball: 14 mm above the middle, where the surface has
/// turned half away; and 12 mm either side of it, on the bead and opposite the bead.
const ABOVE: (u32, u32) = (48, 23);
const ON_THE_BEAD: (u32, u32) = (69, 48);
const OPPOSITE_THE_BEAD: (u32, u32) = (26, 48);

#[test]
fn a_solid_surface_hides_what_is_behind_it() {
    let Some(rgba) = ball_from(NamedView::Anterior, Look::Surface) else {
        return;
    };
    assert_eq!(
        rgba[..4],
        [0, 0, 0, 255],
        "outside the volume is opaque black"
    );
    let middle = at(&rgba, MIDDLE);
    assert!(
        (40..=SHELL).contains(&middle),
        "the lit shell, not the core behind it: {middle}"
    );
}

#[test]
fn a_solid_surface_is_darker_where_it_turns_away() {
    let Some(rgba) = ball_from(NamedView::Anterior, Look::Surface) else {
        return;
    };
    let (middle, above) = (at(&rgba, MIDDLE), at(&rgba, ABOVE));
    assert!(above > 0, "it is still lit a little");
    // Half turned away, it has two thirds of the light: clearly darker, not a shade darker.
    assert!(
        u32::from(above) * 5 < u32::from(middle) * 4,
        "{above} above, {middle} in the middle"
    );
}

#[test]
fn a_solid_bead_stands_out_in_front_and_is_hidden_from_behind() {
    let Some(front) = ball_from(NamedView::Anterior, Look::Surface) else {
        return;
    };
    let (bead, opposite) = (at(&front, ON_THE_BEAD), at(&front, OPPOSITE_THE_BEAD));
    assert!(
        bead > opposite,
        "{bead} on the bead, {opposite} opposite it"
    );
    let Some(behind) = ball_from(NamedView::Posterior, Look::Surface) else {
        return;
    };
    assert!(
        brightest(&behind) <= 70,
        "nothing shines through: {}",
        brightest(&behind)
    );
}

#[test]
fn a_solid_picture_at_half_quality_is_nearly_the_same() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    let full = scene_of(&volume, NamedView::Anterior, Look::Surface);
    let coarse = SceneParams {
        quality: 0.5,
        ..full.clone()
    };
    let apart = mean_difference(&drawn(&mut gpu, &full), &drawn(&mut gpu, &coarse));
    assert!(apart < 6.0, "{apart} grey levels apart on average");
}

#[test]
fn a_solid_picture_is_the_same_each_time_it_is_drawn() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    let scene = scene_of(&volume, NamedView::Left, Look::Surface);
    assert_eq!(
        drawn(&mut gpu, &scene),
        drawn(&mut gpu, &scene),
        "the pattern is fixed"
    );
}

#[test]
fn a_solid_surface_is_not_hidden_by_the_noise_in_the_air_around_it() {
    use voxa_core::Volume;
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    // The ball, with the faint speckle a scanner leaves in the air about a patient: every
    // empty voxel given a value up to a twentieth of the way up the window.
    let clean = ball();
    let speckled = clean.values().iter().enumerate().map(|(at, value)| {
        let speckle = (at.wrapping_mul(2_654_435_761) >> 7) % 21;
        if *value > 0.0 {
            *value
        } else {
            f32::from(u8::try_from(speckle).expect("under 21"))
        }
    });
    let noisy = Volume::new(clean.dims(), speckled.collect(), clean.voxel_to_patient());
    let noisy = noisy.expect("adds up");
    gpu.upload(&noisy).expect("uploads");
    let rgba = drawn(
        &mut gpu,
        &scene_of(&noisy, NamedView::Anterior, Look::Surface),
    );
    let middle = at(&rgba, MIDDLE);
    assert!(
        (40..=SHELL).contains(&middle),
        "the shell is seen through the air: {middle}"
    );
    assert!(at(&rgba, (8, 8)) <= 2, "and the air itself is not drawn");
}
