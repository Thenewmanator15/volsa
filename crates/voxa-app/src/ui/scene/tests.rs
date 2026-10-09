//! The 3D viewport drawn with no window and no graphics device.

use super::*;
use crate::scene::Scene;

/// Draw the scene once, at the size it believes it has, and give what it asked for.
fn shown(state: &AppState) -> Vec<Action> {
    let ctx = egui::Context::default();
    let mut view = SceneView::new(None);
    let mut actions = Vec::new();
    let area = Rect::from_min_size(pos2(10.0, 20.0), Scene::FIRST_VIEWPORT.into());
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        show(ui, area, state, None, &mut view, None, &mut actions);
    });
    output.textures_delta.clear();
    assert!(!view.has_picture(), "there is no device to draw one with");
    actions
}

#[test]
fn the_scene_is_drawn_in_every_state_without_a_graphics_device_and_asks_for_nothing() {
    let unavailable =
        SceneStatus::Unavailable(vec!["This series cannot be shown in 3D.".to_owned()]);
    let building = SceneStatus::Building { done: 3, of: 30 };
    for status in [
        SceneStatus::Empty,
        building,
        SceneStatus::Ready,
        unavailable,
    ] {
        let mut state = AppState::default();
        state.scene.status = status;
        assert!(shown(&state).is_empty(), "{:?}", state.scene.status);
    }
}

#[test]
fn a_scene_laid_out_at_a_new_size_says_so() {
    let mut state = AppState::default();
    state.scene.viewport = [10.0, 10.0];
    let viewport = Scene::FIRST_VIEWPORT;
    assert_eq!(shown(&state), [Action::SceneResized { viewport }]);
}
