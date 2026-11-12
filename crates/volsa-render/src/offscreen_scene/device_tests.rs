//! Work the graphics device does in place of the processor: the largest value in each block
//! of a volume, and the curve through its values. Each is held to what the processor makes.

use half::f16;
use volsa_core::{Affine, Point, Volume, WindowLevel};
use volsa_volume::{PlaneKind, plane_of};

use crate::camera::{Camera, NamedView};
use crate::plane_view::PlaneView;
use crate::prepared::{MAX_VOLUME_BYTES, Prepared};
use crate::scene::{Look, SceneParams};
use crate::smoothed::Smoothed;
use crate::testing::headless_scene;

/// A volume whose sides are not whole blocks, of values scattered between 0 and 1000 with a
/// bright streak, a value that is not a number and one that is infinite.
fn scattered() -> Volume {
    let dims = [21_u32, 19, 17];
    let count = dims.iter().product::<u32>();
    let mut values: Vec<f32> = (0..count)
        .map(|at| {
            let mixed = at.wrapping_mul(2_654_435_761) >> 8;
            let streak = if at % 21 == at / 399 { 600.0 } else { 0.0 };
            (mixed % 400) as f32 + streak
        })
        .collect();
    values[5] = f32::NAN;
    values[4000] = f32::INFINITY;
    let placed = Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.2, 0.0], [0.0, 0.0, 2.0], [0.0; 3]);
    Volume::new(dims, values, placed).expect("adds up")
}

/// A smooth hill of values, 24 by 20 voxels in 18 slices: nothing sharp for a curve to
/// overshoot at, so that two ways of working the curve out can be held close together.
fn hill() -> Volume {
    let dims = [24_u32, 20, 18];
    let mut values = Vec::new();
    for slice in 0..dims[2] {
        for row in 0..dims[1] {
            for col in 0..dims[0] {
                let away = |at: u32, of: u32| (f64::from(at) / f64::from(of - 1) - 0.5).powi(2);
                let from_middle = away(col, dims[0]) + away(row, dims[1]) + away(slice, dims[2]);
                #[expect(clippy::cast_possible_truncation, reason = "a small made-up value")]
                values.push((900.0 * (-6.0 * from_middle).exp()) as f32);
            }
        }
    }
    let placed = Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 2.5], [0.0; 3]);
    Volume::new(dims, values, placed).expect("adds up")
}

fn ready(volume: &Volume) -> (Prepared, Prepared) {
    let whole = Prepared::of(volume, 2048, MAX_VOLUME_BYTES);
    let light = Prepared::without_blocks(volume, 2048, MAX_VOLUME_BYTES);
    (whole, light)
}

#[test]
fn a_volume_can_be_made_ready_without_its_blocks() {
    let (whole, light) = ready(&scattered());
    assert!(whole.maxima.is_some() && light.maxima.is_none());
    assert_eq!(
        light.halves, whole.halves,
        "the values are made ready all the same"
    );
    assert_eq!((light.dims, light.blocks), (whole.dims, whole.blocks));
    assert_eq!((light.smallest, light.span), (whole.smallest, whole.span));
}

#[test]
fn where_the_device_cannot_the_blocks_come_from_the_values_as_they_are_stored() {
    let (whole, light) = ready(&scattered());
    let here = crate::on_device::largest_in_blocks(&light.halves, light.dims);
    assert_eq!(Some(here), whole.maxima, "block for block, bit for bit");
    assert_eq!(
        crate::on_device::largest_in_blocks(&[], [0; 3]),
        Vec::<u16>::new()
    );
}

#[test]
fn the_blocks_the_device_works_out_are_the_ones_the_processor_works_out() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let (whole, light) = ready(&scattered());
    assert!(gpu.makes_blocks(), "this device can");
    gpu.upload_prepared(&light).expect("uploads");
    let held = gpu.blocks_as_held().expect("a volume is held");
    assert_eq!(Some(held), whole.maxima, "block for block, bit for bit");
}

#[test]
fn a_volume_whose_blocks_the_device_worked_out_is_drawn_the_same() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = scattered();
    let (whole, light) = ready(&volume);
    let size = [64_u32, 48];
    let camera = Camera::fitted(NamedView::Anterior, &volume.corners(), [64.0, 48.0]);
    let scene = |look| SceneParams {
        camera,
        window: WindowLevel::new(500.0, 1000.0),
        look,
        cuts: Vec::new(),
        slice: None,
        quality: 1.0,
        skip_empty: true,
    };
    for look in [Look::Surface, Look::Vr, Look::Mip] {
        gpu.upload_prepared(&whole).expect("uploads");
        let by_processor = gpu.render(&scene(look), size).expect("renders");
        gpu.upload_prepared(&light).expect("uploads");
        let by_device = gpu.render(&scene(look), size).expect("renders");
        assert!(by_device == by_processor, "{look:?}");
        assert!(
            by_device.iter().any(|byte| *byte > 40),
            "and shows something"
        );
    }
}

/// The three planes through the middle of a volume, 61 by 61 pixels each.
fn planes(volume: &Volume) -> Vec<PlaneView> {
    let placement = volume.voxel_to_patient();
    let [cols, rows, slices] = volume.dims().map(|count| f64::from(count - 1) / 2.0);
    let middle = placement.apply(Point::new(cols, rows, slices));
    let view = |kind| {
        let plane = plane_of(&placement, volume.dims(), kind, middle, true).expect("a plane");
        PlaneView::showing(plane, &volume.corners(), [61.0; 2], 1.0, [0.0; 2])
    };
    PlaneKind::ALL.into_iter().map(view).collect()
}

#[test]
fn the_curve_the_device_makes_is_the_curve_the_processor_makes() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = hill();
    gpu.upload(&volume).expect("uploads");
    let by_processor = Smoothed::of(&volume, 2048, MAX_VOLUME_BYTES);
    assert!(gpu.makes_curves(), "this device can");
    assert!(gpu.smooth_planes_on_device(), "there is room for it");
    let held = gpu.curve_as_held().expect("a curve is held");
    let real = |bits: &u16| f16::from_bits(*bits).to_f32();
    let apart = held
        .iter()
        .zip(by_processor.halves())
        .map(|(a, b)| (real(a) - real(b)).abs());
    // The device starts from the values as it holds them, in 16 bits; the processor from
    // the values themselves. Of the span from the smallest value to the largest:
    let furthest = apart.fold(0.0_f32, f32::max);
    assert!(furthest < 0.004, "furthest apart: {furthest}");
    assert_eq!(held.len(), by_processor.halves().len());
    // And the pictures drawn from the two are the same to the eye.
    let window = WindowLevel::new(450.0, 900.0);
    for view in planes(&volume) {
        let from_device = gpu.render_plane(&view, window, [61; 2]).expect("renders");
        assert!(gpu.smooth_planes(Some(&by_processor)));
        let from_processor = gpu.render_plane(&view, window, [61; 2]).expect("renders");
        assert!(gpu.smooth_planes_on_device());
        let greys = from_device.iter().zip(&from_processor).step_by(4);
        let off = greys.map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
        assert!(off <= 1, "a grey level at most: {off}");
        assert!(from_device.iter().step_by(4).any(|grey| *grey > 100));
    }
}

#[test]
fn between_slices_the_curve_the_device_makes_is_on_the_bend() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    // 6 by 6 voxels in 16 slices, each slice the square of its number: it bends between
    // slices, which a blend cuts across. Half way between slices 4 and 5 it is 20.25.
    let values = (0..16u16).flat_map(|slice| vec![f32::from(slice * slice); 36]);
    let volume = Volume::new([6, 6, 16], values.collect(), Affine::identity()).expect("adds up");
    gpu.upload(&volume).expect("uploads");
    let plane = plane_of(
        &volume.voxel_to_patient(),
        volume.dims(),
        PlaneKind::Sagittal,
        Point::new(2.5, 2.5, 7.5),
        true,
    );
    let view = PlaneView {
        plane: plane.expect("a plane"),
        mm_per_px: 0.5,
        offset_mm: [0.0; 2],
    };
    let narrow = WindowLevel::new(20.25, 10.0);
    let grey = |rgba: &[u8], row: usize| rgba[(row * 13 + 6) * 4];
    let plain = gpu.render_plane(&view, narrow, [13, 33]).expect("renders");
    assert!(gpu.smooth_planes_on_device());
    let smooth = gpu.render_plane(&view, narrow, [13, 33]).expect("renders");
    // Row 22 of the picture is half way between slices 4 and 5.
    assert!(
        grey(&smooth, 22).abs_diff(128) <= 2,
        "{}",
        grey(&smooth, 22)
    );
    assert!(grey(&plain, 22).abs_diff(128) >= 5, "{}", grey(&plain, 22));
}

#[test]
fn with_no_volume_on_the_device_there_is_no_curve_to_make() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    assert!(!gpu.smooth_planes_on_device());
    assert_eq!(gpu.curve_as_held(), None);
    assert_eq!(gpu.blocks_as_held(), None);
}

#[test]
fn a_device_with_nothing_else_in_hand_is_timed_over_a_piece_of_work() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let nothing = |gpu: &crate::offscreen_scene::HeadlessScene| {
        let describe = egui_wgpu::wgpu::CommandEncoderDescriptor::default();
        gpu.gpu.device.create_command_encoder(&describe).finish()
    };
    let timed = |gpu: &crate::offscreen_scene::HeadlessScene| {
        crate::timed_submit(&gpu.gpu.device, &gpu.gpu.queue, nothing(gpu))
    };
    gpu.settle();
    assert!(timed(&gpu).is_some(), "idle: timed");
    // After a volume and the curve through it have been handed over and waited for.
    let values = (0..48 * 48 * 48).map(|at| (at % 251) as f32).collect();
    let volume = Volume::new([48; 3], values, Affine::identity()).expect("adds up");
    gpu.upload(&volume).expect("uploads");
    assert!(gpu.smooth_planes_on_device());
    gpu.settle();
    assert!(timed(&gpu).is_some(), "idle again");
}

#[test]
fn a_volume_handed_to_the_device_beforehand_from_another_thread_is_drawn_the_same() {
    let Some(mut gpu) = headless_scene() else {
        return;
    };
    let volume = scattered();
    let camera = Camera::fitted(NamedView::Anterior, &volume.corners(), [64.0, 48.0]);
    let scene = SceneParams {
        camera,
        window: WindowLevel::new(500.0, 1000.0),
        look: Look::Mip,
        cuts: Vec::new(),
        slice: None,
        quality: 1.0,
        skip_empty: true,
    };
    let (_, light) = ready(&volume);
    assert!(!light.is_handed_over());
    gpu.upload_prepared(&light).expect("uploads");
    let as_before = gpu.render(&scene, [64, 48]).expect("renders");
    // Made ready and handed over on another thread, as a worker does it.
    let (device, queue) = gpu.device();
    let early = std::thread::scope(|threads| {
        let handing = threads.spawn(|| {
            let mut early = Prepared::without_blocks(&volume, 2048, MAX_VOLUME_BYTES);
            assert!(early.hand_over(&device, &queue), "there is room");
            early
        });
        handing.join().expect("the thread ends")
    });
    assert!(early.is_handed_over());
    gpu.upload_prepared(&early).expect("taken up");
    assert!(!early.is_handed_over(), "taken up once");
    let handed_early = gpu.render(&scene, [64, 48]).expect("renders");
    assert!(handed_early == as_before, "pixel for pixel");
    assert!(
        handed_early.iter().any(|byte| *byte > 40),
        "and shows something"
    );
    // Given a second time it is handed over in the ordinary way, and still the same.
    gpu.upload_prepared(&early).expect("uploads");
    let again = gpu.render(&scene, [64, 48]).expect("renders");
    assert!(again == as_before, "the same picture once more");
}
