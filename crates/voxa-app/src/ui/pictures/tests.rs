//! The pictures of the arrangement shown: with no 3D view among them, the scene's volume still
//! reaches the graphics card, and no second copy of it waits in memory.

use egui::{Pos2, Rect, vec2};
use voxa_render::{MAX_VOLUME_BYTES, Prepared};

use super::*;
use crate::pictures::Pictures;
use crate::testing::{a_render_state, cube, thick_scan_ready};

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

/// The state of the made-up scan, ready in the scene and with this document open.
fn with_a_document_open(open: bool) -> AppState {
    let mut state = thick_scan_ready();
    if open {
        let entry = "DOCS/A0001".to_owned();
        state.document = Some(crate::state::OpenDocument {
            entry,
            report: None,
        });
    }
    state
}

#[test]
fn with_a_document_open_the_volume_made_ready_reaches_the_card_and_no_copy_of_it_waits() {
    let Some(render) = a_render_state() else {
        return;
    };
    let side = render.device.limits().max_texture_dimension_3d;
    let series = crate::testing::id(crate::testing::THICK_SCAN);
    let volume = cube(8, 1.0);
    for open in [true, false] {
        // A worker made the volume ready while the report was being read, or not open at all.
        let ready = Prepared::of(&volume, side, MAX_VOLUME_BYTES);
        let state = with_a_document_open(open);
        let session = Session::holding(state, &series, Arc::clone(&volume), Some(ready));
        let mut view = scene::SceneView::new(Some(&render));
        let mut planes = planes::PlanesView::new(Some(&render));
        let mut actions = Vec::new();
        let held = settle_volume(
            &session,
            (&mut view, &mut planes),
            Some(&render),
            &mut actions,
        );
        assert_eq!(
            held.as_ref(),
            Some(&volume),
            "the volume of the scene's series"
        );
        if open {
            // No picture is drawn to take it up, so it is put on the card here.
            assert!(!view.has_waiting(), "no second copy waits in memory");
            assert!(!view.needs(&volume), "the card holds this volume");
            assert_eq!(actions, [Action::SceneUploaded { halved: 0 }]);
        } else {
            // The pictures take it up as they are drawn.
            assert!(view.has_waiting(), "handed over, for the scene to take");
            assert!(view.needs(&volume));
            assert_eq!(actions, []);
        }
        // Nothing is left in the store either way: it was taken once.
        assert!(session.take_prepared(&series).is_none());
    }
}
