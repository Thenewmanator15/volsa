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
        show(ui, area, state, (None, None), &mut view, None, &mut actions);
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

/// Draw the scene over several frames, the mouse doing in each what `frames` says, and give
/// what the last frame asked for.
fn after(state: &AppState, frames: Vec<Vec<egui::Event>>) -> Vec<Action> {
    let ctx = egui::Context::default();
    let mut view = SceneView::new(None);
    let area = Rect::from_min_size(pos2(10.0, 20.0), Scene::FIRST_VIEWPORT.into());
    let mut last = Vec::new();
    for events in [vec![Vec::new()], frames].concat() {
        last.clear();
        let input = egui::RawInput {
            events,
            ..egui::RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            show(ui, area, state, (None, None), &mut view, None, &mut last);
        });
        output.textures_delta.clear();
    }
    last
}

/// A press of the main button at one place, then the pointer moved to another with it held.
fn drag(from: [f32; 2], to: [f32; 2]) -> Vec<Vec<egui::Event>> {
    let at = |[x, y]: [f32; 2]| pos2(x, y);
    let press = egui::Event::PointerButton {
        pos: at(from),
        button: PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    };
    vec![
        vec![egui::Event::PointerMoved(at(from)), press],
        vec![egui::Event::PointerMoved(at(to))],
    ]
}

/// A ready scene seen from the patient's left at half a millimetre a point, with one cut
/// through the middle whose normal points to the left of the screen. The middle of the
/// scene, where the cut's handle is, is at (410, 320) in the window; its knob at (350, 320).
fn cut_open() -> AppState {
    use voxa_core::Point;
    use voxa_render::{Camera, CutPlane, NamedView};
    let mut state = AppState::default();
    let middle = Point::new(0.0, 0.0, 0.0);
    state.scene.status = SceneStatus::Ready;
    state.scene.camera = Camera::looking(NamedView::Left, middle, 0.5);
    state.scene.cuts = vec![CutPlane {
        point: middle,
        normal: [0.0, -1.0, 0.0],
    }];
    state
}

#[test]
fn dragging_a_cuts_handle_moves_the_cut_with_the_pointer_and_does_not_turn_the_scene() {
    let asked = after(&cut_open(), drag([410.0, 320.0], [390.0, 320.0]));
    // 20 points along the normal as seen is 10 mm along it: 10 mm less is removed.
    let (cut, by_mm) = (0, -10.0);
    assert_eq!(asked, [Action::CutMove { cut, by_mm }]);
}

#[test]
fn dragging_a_cuts_knob_turns_the_cut() {
    let asked = after(&cut_open(), drag([350.0, 320.0], [350.0, 300.0]));
    let (cut, by) = (0, [0.0, -20.0]);
    assert_eq!(asked, [Action::CutTurn { cut, by }]);
}

#[test]
fn dragging_anywhere_else_still_turns_the_scene() {
    let asked = after(&cut_open(), drag([600.0, 500.0], [610.0, 505.0]));
    assert_eq!(asked, [Action::SceneOrbit { by: [10.0, 5.0] }]);
}

#[test]
fn dragging_the_slices_handle_moves_through_the_slices() {
    use voxa_core::Point;
    use voxa_render::{Camera, NamedView};

    use crate::reducer::reduce;
    use crate::testing::{THICK_SCAN, id, three_series};
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    let (viewport, series) = (0, id(THICK_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    reduce(&mut state, Action::ShowSliceInScene(true));
    state.scene.status = SceneStatus::Ready;
    // The slice shown is the sixteenth, 75 mm up; its middle is put in the middle of the
    // scene, seen from the front, where up the screen is up the stack.
    let middle = Point::new(1.5, 1.5, 75.0);
    state.scene.camera = Camera::looking(NamedView::Anterior, middle, 0.5);
    // Seen from the front the slice is on its edge, a line across the middle of the scene,
    // and its handle is half way along that line. 10 points up is 5 mm, the distance to the
    // next slice.
    let asked = after(&state, drag([410.0, 320.0], [410.0, 310.0]));
    let (viewport, delta) = (0, 1);
    assert_eq!(asked, [Action::Scroll { viewport, delta }]);
    // From below it is seen whole, 8 points square, and the handle is in the middle of its
    // front edge, 4 points above the middle of the scene. Moving it across moves no slice.
    state.scene.camera = Camera::looking(NamedView::Inferior, middle, 0.5);
    let asked = after(&state, drag([410.0, 316.0], [440.0, 316.0]));
    assert_eq!(
        asked,
        [],
        "the slice lies flat to the screen: there is nowhere to drag it"
    );
    state.scene.camera = Camera::looking(NamedView::Anterior, middle, 0.5);
    reduce(&mut state, Action::ShowSliceInScene(false));
    let asked = after(&state, drag([410.0, 320.0], [410.0, 310.0]));
    assert_eq!(
        asked,
        [Action::SceneOrbit { by: [0.0, -10.0] }],
        "no slice, no handle"
    );
}

/// The 30-slice series ready in the scene, seen from the patient's left at half a millimetre
/// a point with the position in the middle of the scene, which is at (410, 320) in the
/// window. Up the screen is up the stack, and the back of the head is to the right.
fn with_planes(pictures: crate::pictures::Pictures) -> AppState {
    use voxa_core::Point;
    use voxa_render::{Camera, NamedView};

    use crate::reducer::reduce;
    use crate::testing::{THICK_SCAN, id, three_series};
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    let (viewport, series) = (0, id(THICK_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let placement = info.voxel_to_patient.expect("placed");
    let end = |i: usize, bit: usize, count: f64| if i & bit == 0 { -0.5 } else { count - 0.5 };
    let corners = std::array::from_fn(|i| {
        placement.apply(Point::new(end(i, 1, 4.0), end(i, 2, 4.0), end(i, 4, 30.0)))
    });
    let series = id(THICK_SCAN);
    reduce(&mut state, Action::VolumeReady { series, corners });
    reduce(&mut state, Action::SetPictures(pictures));
    let position = state.planes.position.expect("the volume is ready");
    state.scene.camera = Camera::looking(NamedView::Left, position, 0.5);
    state
}

#[test]
fn dragging_the_end_of_an_arm_of_the_cross_moves_that_plane_through_its_layers() {
    use voxa_volume::PlaneKind;

    use crate::pictures::Pictures;
    let state = with_planes(Pictures::Planes);
    // The arm along the axial plane's normal stands 28 points up from the position. Slices
    // are 5 mm apart: 10 points.
    let (grip, half_way, a_layer) = ([410.0, 292.0], [410.0, 287.0], [410.0, 282.0]);
    assert_eq!(
        after(&state, drag(grip, half_way)),
        [],
        "half a layer is not a layer yet"
    );
    let further = vec![egui::Event::PointerMoved(pos2(410.0, 282.0))];
    let asked = after(&state, [drag(grip, half_way), vec![further]].concat());
    let (kind, steps) = (PlaneKind::Axial, 1);
    assert_eq!(
        asked,
        [Action::PlaneScroll { kind, steps }],
        "up the stack is on to later slices"
    );
    let asked = after(&state, drag(grip, [410.0, 322.0]));
    let steps = -3;
    assert_eq!(
        asked,
        [Action::PlaneScroll { kind, steps }],
        "30 points down is three back"
    );
    // The coronal plane's arm lies across the screen, to the back of the head.
    let asked = after(&state, drag([438.0, 320.0], [448.0, 320.0]));
    assert!(
        matches!(
            asked[..],
            [Action::PlaneScroll {
                kind: PlaneKind::Coronal,
                ..
            }]
        ),
        "{asked:?}"
    );
    let _ = a_layer;
}

#[test]
fn with_the_planes_out_of_sight_there_is_no_cross_to_take_hold_of() {
    let state = with_planes(crate::pictures::Pictures::Scene);
    let asked = after(&state, drag([410.0, 292.0], [410.0, 282.0]));
    assert_eq!(asked, [Action::SceneOrbit { by: [0.0, -10.0] }]);
}

#[test]
fn in_a_narrow_picture_the_words_go_beneath_the_letter_and_take_the_whole_width() {
    use super::captions::caption_room;
    let (down, wide) = caption_room(1000.0);
    assert_eq!(
        (down, wide),
        (0.0, 486.0),
        "beside the letter, in half the width"
    );
    let (down, wide) = caption_room(376.0);
    assert_eq!(wide, 376.0, "a quarter of the window: the whole width");
    assert!(down >= 18.0, "and beneath the letter: {down}");
}
