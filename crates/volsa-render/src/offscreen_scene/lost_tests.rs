//! A graphics device that stops answering, as after a driver reset: nothing is drawn on it,
//! the drawing says so, and nothing panics. The device is destroyed, which a test may do; a
//! real reset is never provoked.

use std::panic::{AssertUnwindSafe, catch_unwind};

use egui_wgpu::wgpu;
use voxa_core::{Patient, Point, Volume};
use voxa_volume::{PlaneKind, plane_of};

use super::*;
use crate::camera::NamedView;
use crate::plane_view::PlaneView;
use crate::prepared::{MAX_VOLUME_BYTES, Prepared};
use crate::scene::Look;
use crate::testing::{SIDE, ball, ball_labels, ball_window, headless_scene, scene_of};

const PICTURE: [u32; 2] = [SIDE, SIDE];

/// A scene holding the ball and its labels, and the ball. `None` with no adapter.
fn holding_the_ball() -> Option<(HeadlessScene, Volume)> {
    let mut gpu = headless_scene()?;
    let volume = ball();
    gpu.upload(&volume).expect("uploads");
    assert!(gpu.label(Some(&ball_labels([0.0; 3]))));
    Some((gpu, volume))
}

fn through_the_middle(volume: &Volume) -> PlaneView {
    let middle: Point<Patient> = Point::new(0.0, 0.0, 0.0);
    let placement = volume.voxel_to_patient();
    let plane = plane_of(&placement, volume.dims(), PlaneKind::Axial, middle, true);
    let panel = [SIDE as f32; 2];
    PlaneView::showing(
        plane.expect("a plane"),
        &volume.corners(),
        panel,
        1.0,
        [0.0; 2],
    )
}

fn lost(drawn: &Result<Vec<u8>, RenderError>) -> bool {
    matches!(drawn, Err(RenderError::DeviceLost))
}

#[test]
fn a_scene_on_a_lost_device_is_not_drawn_and_that_is_said() {
    let Some((mut gpu, volume)) = holding_the_ball() else {
        return;
    };
    let drawn = |gpu: &mut HeadlessScene, look| {
        gpu.render(&scene_of(&volume, NamedView::Anterior, look), PICTURE)
    };
    let picture = drawn(&mut gpu, Look::Mip).expect("draws");
    assert_eq!(picture.len(), (SIDE * SIDE * 4) as usize);
    gpu.gpu.lose();
    for look in [Look::Mip, Look::Surface, Look::Vr] {
        assert!(lost(&drawn(&mut gpu, look)), "{look:?}");
    }
    assert!(gpu.gpu.is_lost());
}

#[test]
fn a_plane_on_a_lost_device_is_not_drawn_and_that_is_said() {
    let Some((mut gpu, volume)) = holding_the_ball() else {
        return;
    };
    let view = through_the_middle(&volume);
    assert!(gpu.render_plane(&view, ball_window(), PICTURE).is_ok());
    gpu.gpu.lose();
    assert!(lost(&gpu.render_plane(&view, ball_window(), PICTURE)));
}

#[test]
fn labels_on_a_lost_device_are_not_drawn_and_none_can_be_put_there() {
    let Some((mut gpu, volume)) = holding_the_ball() else {
        return;
    };
    let parts = scene_of(&volume, NamedView::Anterior, Look::Parts);
    assert!(gpu.render_parts(&parts, PICTURE).is_ok());
    assert!(gpu.has_parts());
    gpu.gpu.lose();
    assert!(lost(&gpu.render_parts(&parts, PICTURE)));
    assert!(!gpu.label(Some(&ball_labels([0.0; 3]))), "no room on it");
    assert!(!gpu.has_parts());
    // Looks and overlays are only set down for the next time, and are set down as ever.
    gpu.overlay(crate::labels::Overlay::default());
    gpu.set_looks(&[], None);
    assert!(!gpu.label(None));
}

#[test]
fn what_is_asked_of_a_lost_device_again_and_again_is_not_a_reason_to_stop() {
    let Some((mut gpu, volume)) = holding_the_ball() else {
        return;
    };
    let scene = scene_of(&volume, NamedView::Anterior, Look::Mip);
    gpu.gpu.lose();
    let ready = Prepared::of(&volume, 2048, MAX_VOLUME_BYTES);
    for _ in 0..3 {
        // Handing it a volume is refused, and curves, blocks and waiting on it carry on, all
        // without a panic.
        assert!(matches!(gpu.upload(&volume), Err(RenderError::DeviceLost)));
        assert!(matches!(
            gpu.upload_prepared(&ready),
            Err(RenderError::DeviceLost)
        ));
        let _ = gpu.smooth_planes_on_device();
        let _ = (gpu.makes_blocks(), gpu.makes_curves());
        gpu.settle();
        assert!(lost(&gpu.render(&scene, PICTURE)));
    }
}

#[test]
fn a_scene_device_that_answers_stops_a_test_at_what_it_refuses_not_after() {
    let Some((gpu, _)) = holding_the_ball() else {
        return;
    };
    // A buffer that is to be used for nothing is not allowed.
    let refused = catch_unwind(AssertUnwindSafe(|| {
        let _ = gpu.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("made-up"),
            size: 16,
            usage: wgpu::BufferUsages::empty(),
            mapped_at_creation: false,
        });
    }));
    assert!(refused.is_err(), "the refusal was to stop the test there");
    assert!(!gpu.gpu.is_lost());
}
