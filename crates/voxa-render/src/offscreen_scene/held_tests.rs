//! A held volume handed to the device as stored, a slab at a time, draws as the same values
//! put there as 32-bit numbers do.

use std::sync::Arc;

use voxa_core::{HeldVolume, Rescale, SliceIn, Stored, Volume};

use super::*;
use crate::camera::NamedView;
use crate::scene::Look;
use crate::testing::{SIDE, ball, headless_scene, mean_difference, scene_of};
use crate::upload::{CardRoom, UploadState};

const PICTURE: [u32; 2] = [SIDE, SIDE];
const CT: Rescale = Rescale {
    slope: 1.0,
    intercept: -1024.0,
};

/// The test ball held as a CT holds its values: whole numbers `stored` as said, with the
/// CT's rescale (unsigned) or none (signed), in slabs of `slab_bytes`.
fn held_ball(stored: Stored, slab_bytes: usize) -> Arc<HeldVolume> {
    let volume = ball();
    let rescale = if stored == Stored::Signed16 {
        Rescale::default()
    } else {
        CT
    };
    let held = HeldVolume::new(
        volume.dims(),
        volume.voxel_to_patient(),
        stored,
        rescale,
        slab_bytes,
    )
    .expect("a volume");
    for slice in 0..volume.dims()[2] {
        let values = volume.slice_values(slice).expect("a slice");
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the ball's values are whole"
        )]
        let whole = |value: &f32| *value as i32;
        let put = match stored {
            Stored::Unsigned16 => {
                let numbers: Vec<u16> = values
                    .iter()
                    .map(|v| u16::try_from(whole(v) + 1024).expect("in range"))
                    .collect();
                held.put_slice(slice, SliceIn::Unsigned(&numbers))
            }
            Stored::Signed16 => {
                let numbers: Vec<i16> = values
                    .iter()
                    .map(|v| i16::try_from(whole(v)).expect("in range"))
                    .collect();
                held.put_slice(slice, SliceIn::Signed(&numbers))
            }
            Stored::Float32 => held.put_slice(slice, SliceIn::Float(values)),
        };
        put.expect("fits");
    }
    Arc::new(held)
}

/// The ball's values as a CT holding them unsigned reads them: 1024 below.
fn ball_in_hounsfield() -> Volume {
    let volume = ball();
    let values = volume.values().to_vec();
    Volume::new(volume.dims(), values, volume.voxel_to_patient()).expect("adds up")
}

fn pictures(gpu: &mut HeadlessScene, volume: &Volume) -> Vec<Vec<u8>> {
    [Look::Vr, Look::Mip]
        .into_iter()
        .map(|look| {
            let mut scene = scene_of(volume, NamedView::Anterior, look);
            if look == Look::Vr {
                scene.window = voxa_core::WindowLevel::new(200.0, 400.0);
            }
            gpu.render(&scene, PICTURE).expect("draws")
        })
        .collect()
}

#[test]
fn a_held_ct_draws_as_the_old_volume_did() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let plain = ball_in_hounsfield();
    gpu.upload(&plain).expect("uploads");
    let old = pictures(&mut gpu, &plain);
    for stored in [Stored::Unsigned16, Stored::Signed16, Stored::Float32] {
        let held = held_ball(stored, 1 << 20);
        assert_eq!(gpu.upload_held(&held), Ok(0), "{stored:?}");
        let new = pictures(&mut gpu, &plain);
        for (old, new) in old.iter().zip(&new) {
            let off = mean_difference(old, new);
            assert!(off < 1.0, "{stored:?}: {off} grey levels apart on average");
        }
    }
}

#[test]
fn paced_hand_over_finishes_in_steps() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    // 48 x 48 x 48 values of 2 bytes, in slabs of 8 slices: six slabs of 36 KiB.
    let held = held_ball(Stored::Unsigned16, 48 * 48 * 2 * 8);
    let (device, queue) = (&gpu.gpu.device, &gpu.gpu.queue);
    let room = CardRoom::of(device, None);
    gpu.pipeline
        .start_upload(device, Arc::clone(&held), room)
        .expect("room");
    let mut steps = Vec::new();
    loop {
        let state = gpu
            .pipeline
            .continue_upload(device, queue, 2 * 48 * 48 * 2 * 8);
        let done = state == UploadState::Done;
        steps.push(state);
        if done || steps.len() > 10 {
            break;
        }
    }
    let of = 48 * 48 * 48 * 2;
    let going = |done: u64| UploadState::Going { done, of };
    let third = of / 3;
    assert_eq!(steps, [going(third), going(2 * third), UploadState::Done]);
    assert!(gpu.pipeline.has_volume());
}

#[test]
fn a_new_upload_drops_the_one_in_hand() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let plain = ball_in_hounsfield();
    let first = held_ball(Stored::Unsigned16, 48 * 48 * 2 * 8);
    let second = held_ball(Stored::Signed16, 1 << 20);
    let (device, queue) = (&gpu.gpu.device, &gpu.gpu.queue);
    let room = CardRoom::of(device, None);
    gpu.pipeline
        .start_upload(device, first, room)
        .expect("room");
    let one_slab = 48 * 48 * 2 * 8;
    assert!(matches!(
        gpu.pipeline.continue_upload(device, queue, one_slab),
        UploadState::Going { .. }
    ));
    gpu.pipeline
        .start_upload(device, second, room)
        .expect("room");
    assert!(
        !gpu.pipeline.has_volume(),
        "nothing half handed over is drawn"
    );
    assert_eq!(
        gpu.pipeline.continue_upload(device, queue, u64::MAX),
        UploadState::Done
    );
    let drawn = pictures(&mut gpu, &plain);
    gpu.upload_held(&held_ball(Stored::Signed16, 1 << 20))
        .expect("uploads");
    let alone = pictures(&mut gpu, &plain);
    assert_eq!(drawn, alone);
}

#[test]
fn an_upload_to_a_lost_device_fails_without_stopping() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let held = held_ball(Stored::Unsigned16, 1 << 20);
    gpu.gpu.lose();
    assert_eq!(gpu.upload_held(&held), Err(RenderError::DeviceLost));
}
