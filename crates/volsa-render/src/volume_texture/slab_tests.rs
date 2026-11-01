//! A volume handed to the device a slab at a time, and the device that stops answering part
//! way through.

use egui_wgpu::wgpu;

use super::*;
use crate::testing::headless_scene;

/// Six 4 x 4 slices, and slabs of two slices: three slabs.
const DIMS: [u32; 3] = [4, 4, 6];
const TWO_SLICES: usize = 4 * 4 * 2 * 2;
/// 16-bit floating-point 0.5, as stored.
const HALF: u16 = 0x3800;

/// How many times the device was asked whether it answers while a made-up volume was handed
/// to it, with `answers` giving each answer in turn, counting from 1. `None` on a machine
/// with no adapter.
fn asked_while_writing(mut answers: impl FnMut(usize) -> bool) -> Option<usize> {
    let (device, queue) = headless_scene()?.device();
    let values = texture(
        &device,
        "made-up values",
        DIMS,
        wgpu::TextureFormat::R16Float,
    );
    let volume = vec![HALF; 4 * 4 * 6];
    let mut asked = 0;
    let answering = || {
        asked += 1;
        answers(asked)
    };
    write_slabs(&queue, &values, DIMS, &volume, TWO_SLICES, answering);
    Some(asked)
}

#[test]
fn each_slab_is_followed_by_a_question_whether_the_device_still_answers() {
    let Some(asked) = asked_while_writing(|_| true) else {
        return;
    };
    assert_eq!(asked, 3);
}

#[test]
fn the_writing_stops_at_the_first_sign_that_the_device_has_stopped_answering() {
    // It answers once and then does not: the third slab is not written, nor asked about.
    let Some(asked) = asked_while_writing(|asked| asked < 2) else {
        return;
    };
    assert_eq!(asked, 2);
}
