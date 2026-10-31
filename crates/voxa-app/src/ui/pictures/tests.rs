//! The pictures of the arrangement shown: with no 3D view among them, the scene's volume still
//! reaches the graphics card, and no second copy of it waits in memory.

use egui::{Pos2, Rect, vec2};
use voxa_core::Affine;
use voxa_render::{MAX_VOLUME_BYTES, Prepared, RenderError};

use super::*;
use crate::pictures::Pictures;
use crate::testing::thick_scan_ready;

/// The graphics device as the window has it, made on the one adapter every device with no window
/// shares (this test makes no graphics instance of its own); `None` with a printed notice on a
/// computer with no graphics adapter, not even a software one.
fn a_render_state() -> Option<RenderState> {
    match voxa_render::render_state_without_a_window() {
        Ok(render) => Some(render),
        Err(RenderError::NoAdapter) => {
            eprintln!("skipped: no graphics adapter here, not even a software one");
            None
        }
        Err(other) => panic!("{other}"),
    }
}

/// A cube `side` voxels of 1 mm each way, every voxel `value`.
fn cube(side: u32, value: f32) -> Arc<Volume> {
    let values = vec![value; (side * side * side) as usize];
    let placement =
        Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [0.0; 3]);
    Arc::new(Volume::new([side; 3], values, placement).expect("adds up"))
}

#[test]
fn in_slices_the_volume_made_ready_reaches_the_card_and_no_copy_of_it_waits() {
    let Some(render) = a_render_state() else {
        return;
    };
    let state = thick_scan_ready();
    assert_eq!(state.pictures, Pictures::Slices, "as Voxa opens");
    let whole = Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0));
    let area = layout::areas(state.pictures.given(&state.scene.status), whole).scene;
    assert_eq!(area, None, "no 3D view among the slices alone");
    let mut view = scene::SceneView::new(Some(&render));
    let side = render.device.limits().max_texture_dimension_3d;
    let (first, second) = (cube(8, 1.0), cube(12, 2.0));
    let ctx = egui::Context::default();
    for volume in [&first, &second] {
        // A worker made it ready, and the window took it from the store.
        view.offer(Some(Prepared::of(volume, side, MAX_VOLUME_BYTES)));
        assert!(view.has_waiting());
        let mut actions = Vec::new();
        let mut frame = ctx.run_ui(egui::RawInput::default(), |ui| {
            let sources = (Some(volume), None);
            scene_or_card(
                ui,
                area,
                &state,
                sources,
                &mut view,
                Some(&render),
                &mut actions,
            );
        });
        // No renderer is behind this pass to take the font texture.
        frame.textures_delta.clear();
        assert!(!view.has_waiting(), "no second copy waits in memory");
        assert!(!view.needs(volume), "the card holds this volume");
        assert_eq!(actions, [Action::SceneUploaded { halved: 0 }]);
    }
    assert!(view.needs(&first), "and no longer the one before it");
}
