//! The 3D scene in the state: moving its camera, cutting it open, and how it is shown.

use std::path::PathBuf;

use voxa_core::Point;
use voxa_render::{Camera, CutPlane, Look, MAX_CUTS, NamedView};

use super::scene_tests::{corners, ready};
use super::*;
use crate::scene::{MainPanel, Scene};

#[test]
fn a_new_size_refits_an_untouched_camera_and_keeps_what_a_moved_one_shows() {
    let mut state = ready();
    reduce(
        &mut state,
        Action::SceneResized {
            viewport: [400.0, 300.0],
        },
    );
    let fitted = Camera::fitted(NamedView::Anterior, &corners(), [400.0, 300.0]);
    assert_eq!(
        (state.scene.viewport, state.scene.camera),
        ([400.0, 300.0], fitted)
    );

    reduce(&mut state, Action::SceneOrbit { by: [30.0, 10.0] });
    assert!(!state.scene.untouched);
    assert_eq!(state.scene.camera, fitted.orbited([30.0, 10.0]));
    reduce(
        &mut state,
        Action::SceneResized {
            viewport: [1000.0, 900.0],
        },
    );
    // The view is the person's now, and keeps what it shows: as much of the patient across
    // the shorter side of the picture, which is three times as long as it was.
    let kept = fitted.orbited([30.0, 10.0]);
    assert!((state.scene.camera.mm_per_px - fitted.mm_per_px / 3.0).abs() < 1.0e-12);
    assert_eq!(
        (state.scene.camera.yaw, state.scene.camera.pitch),
        (kept.yaw, kept.pitch),
        "and the way it faces"
    );
    assert_eq!(state.scene.camera.target, kept.target);
    assert_eq!(state.scene.viewport, [1000.0, 900.0]);
    reduce(
        &mut state,
        Action::SceneResized {
            viewport: [f32::NAN, 0.0],
        },
    );
    assert_eq!(
        state.scene.viewport,
        [1000.0, 900.0],
        "a size that is no size is ignored"
    );
}

#[test]
fn the_camera_can_be_moved_zoomed_centred_and_set_to_a_named_view() {
    let mut state = ready();
    let start = state.scene.camera;
    reduce(&mut state, Action::ScenePan { by: [10.0, -4.0] });
    reduce(
        &mut state,
        Action::SceneZoom {
            factor: 2.0,
            about: [5.0, 5.0],
        },
    );
    let moved = start.panned([10.0, -4.0]).zoomed_about(2.0, [5.0, 5.0]);
    assert_eq!(state.scene.camera, moved);
    reduce(&mut state, Action::SceneCentre(Point::new(1.0, 2.0, 3.0)));
    assert_eq!(
        state.scene.camera,
        moved.centred_on(Point::new(1.0, 2.0, 3.0))
    );
    reduce(&mut state, Action::SceneView(NamedView::Left));
    let from_the_left = Camera::fitted(NamedView::Left, &corners(), Scene::FIRST_VIEWPORT);
    assert_eq!(
        state.scene.camera, from_the_left,
        "a named view shows the whole volume"
    );
}

#[test]
fn there_are_never_more_than_three_cuts() {
    let mut state = ready();
    reduce(&mut state, Action::SceneOrbit { by: [50.0, 20.0] });
    for _ in 0..=MAX_CUTS {
        reduce(&mut state, Action::CutAdd);
    }
    let facing = CutPlane::facing(&state.scene.camera);
    assert_eq!(state.scene.cuts, vec![facing; MAX_CUTS]);
}

#[test]
fn a_cut_can_be_moved_turned_and_removed() {
    let mut state = ready();
    reduce(&mut state, Action::CutAdd);
    reduce(&mut state, Action::CutAdd);
    let (camera, cut) = (state.scene.camera, CutPlane::facing(&state.scene.camera));
    reduce(&mut state, Action::CutMove { cut: 1, by_mm: 4.0 });
    reduce(
        &mut state,
        Action::CutTurn {
            cut: 1,
            by: [20.0, 0.0],
        },
    );
    assert_eq!(
        state.scene.cuts,
        [cut, cut.moved(4.0).turned(&camera, [20.0, 0.0])]
    );
    let before = state.clone();
    reduce(&mut state, Action::CutRemove { cut: 9 });
    reduce(&mut state, Action::CutMove { cut: 9, by_mm: 1.0 });
    reduce(
        &mut state,
        Action::CutTurn {
            cut: 9,
            by: [1.0, 1.0],
        },
    );
    assert_eq!(state, before, "a cut that is not there is left alone");
    reduce(&mut state, Action::CutRemove { cut: 0 });
    assert_eq!(state.scene.cuts.len(), 1);
}

#[test]
fn the_look_the_slice_and_the_main_panel_are_remembered() {
    let mut state = ready();
    reduce(&mut state, Action::SceneLook(Look::Vr));
    reduce(&mut state, Action::ShowSliceInScene(true));
    reduce(&mut state, Action::SceneUploaded { halved: 1 });
    assert_eq!(
        (
            state.scene.look,
            state.scene.slice_shown,
            state.scene.halved
        ),
        (Look::Vr, true, 1)
    );
    reduce(&mut state, Action::SetMain(MainPanel::Stack));
    assert_eq!(state.main, MainPanel::Stack);
    reduce(&mut state, Action::SetMain(MainPanel::Scene));
    assert_eq!(state.main, MainPanel::Scene);
}

#[test]
fn opening_another_path_empties_the_scene() {
    let mut state = ready();
    reduce(&mut state, Action::SetMain(MainPanel::Stack));
    reduce(
        &mut state,
        Action::SceneResized {
            viewport: [640.0, 480.0],
        },
    );
    let effects = reduce(&mut state, Action::OpenPath(PathBuf::from("other.zip")));
    assert_eq!(effects, [Effect::StartIndex(PathBuf::from("other.zip"))]);
    let empty = Scene {
        viewport: [640.0, 480.0],
        ..Scene::default()
    };
    assert_eq!(state.scene, empty);
    assert_eq!(
        state.main,
        MainPanel::Stack,
        "how the window is laid out is kept"
    );
}

#[test]
fn fitting_also_shows_the_whole_volume_again_while_the_scene_is_the_large_panel() {
    let mut state = ready();
    reduce(&mut state, Action::SceneOrbit { by: [40.0, 10.0] });
    reduce(
        &mut state,
        Action::SceneZoom {
            factor: 5.0,
            about: [30.0, 30.0],
        },
    );
    let turned = state.scene.camera;
    reduce(&mut state, Action::Fit { viewport: 0 });
    let refitted = turned.refitted(&corners(), Scene::FIRST_VIEWPORT);
    assert_eq!(
        state.scene.camera, refitted,
        "the same way round, the whole of it in view"
    );
    assert_ne!(refitted.mm_per_px, turned.mm_per_px);

    reduce(&mut state, Action::SetMain(MainPanel::Stack));
    reduce(
        &mut state,
        Action::SceneZoom {
            factor: 5.0,
            about: [0.0, 0.0],
        },
    );
    let zoomed = state.scene.camera;
    reduce(&mut state, Action::Fit { viewport: 0 });
    assert_eq!(
        state.scene.camera, zoomed,
        "with the slices large, only they are fitted"
    );
}
