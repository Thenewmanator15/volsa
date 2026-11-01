//! The pictures of the arrangement shown: with no 3D view among them, the scene's volume still
//! reaches the graphics card, handed over as drawing it would hand it over.

use egui::{Pos2, Rect, vec2};

use super::*;
use crate::pictures::Pictures;
use crate::testing::{a_render_state, cube, thick_scan_ready};

#[test]
fn in_slices_the_volume_offered_reaches_the_card_and_nothing_of_it_waits() {
    let Some(render) = a_render_state() else {
        return;
    };
    let state = thick_scan_ready();
    assert_eq!(state.pictures, Pictures::Slices, "as Volsa opens");
    let whole = Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0));
    let area = layout::areas(state.pictures.given(&state.scene.status), whole).scene;
    assert_eq!(area, None, "no 3D view among the slices alone");
    let mut view = scene::SceneView::new(Some(&render));
    let (first, second) = (cube(8, 1.0), cube(12, 2.0));
    let ctx = egui::Context::default();
    for volume in [&first, &second] {
        // The window took what the card is to hold from the store: here, the volume itself.
        view.offer(Some((Arc::clone(volume), 0)));
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
        assert!(!view.has_waiting(), "the handing over has begun");
        assert!(
            !view.is_uploading(),
            "and a small volume is there in one frame"
        );
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
fn with_a_document_open_the_volume_reaches_the_card_and_nothing_of_it_waits() {
    let Some(render) = a_render_state() else {
        return;
    };
    let series = crate::testing::id(crate::testing::THICK_SCAN);
    let volume = cube(8, 1.0);
    let ctx = egui::Context::default();
    for open in [true, false] {
        // A worker built the volume while the report was being read, or with none open.
        let state = with_a_document_open(open);
        let session = Session::holding(state, &series, Arc::clone(&volume));
        let mut view = scene::SceneView::new(Some(&render));
        let mut planes = planes::PlanesView::new(Some(&render));
        let mut actions = Vec::new();
        let held = settle_volume(
            &session,
            (&mut view, &mut planes),
            (Some(&render), &ctx),
            &mut actions,
        );
        assert!(
            held.as_ref().is_some_and(|held| Arc::ptr_eq(held, &volume)),
            "the volume of the scene's series"
        );
        if open {
            // No picture is drawn to hand it over, so it is handed over here.
            assert!(!view.has_waiting(), "the handing over has begun");
            assert!(!view.needs(&volume), "the card holds this volume");
            assert_eq!(actions, [Action::SceneUploaded { halved: 0 }]);
        } else {
            // The pictures hand it over as they are drawn.
            assert!(view.has_waiting(), "offered, for the scene to take");
            assert!(view.needs(&volume));
            assert_eq!(actions, []);
        }
    }
}
