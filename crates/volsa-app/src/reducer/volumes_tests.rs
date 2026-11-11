//! Stepping between the volumes of a series that holds several.

use voxa_core::WindowLevel;
use voxa_render::Look;

use super::scene_tests::{build, corners, select};
use super::tests::frames_around;
use super::*;
use crate::scene::SceneStatus;
use crate::testing::{
    FIRST_ECHO, SECOND_ECHO, THICK_SCAN, id, indexed, three_series_files, two_echoes_files,
};

/// The three series and the scan of two echoes listed, the first echo shown.
fn on_the_first_echo() -> AppState {
    let files = [three_series_files(), two_echoes_files()].concat();
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(indexed(files)));
    select(&mut state, 0, FIRST_ECHO);
    state
}

fn step(state: &mut AppState, by: i32) -> Vec<Effect> {
    reduce(state, Action::StepVolume { viewport: 0, by })
}

/// The first echo shown and its volume ready in the scene.
fn first_echo_ready() -> AppState {
    let mut state = on_the_first_echo();
    let (series, corners) = (id(FIRST_ECHO), corners());
    reduce(&mut state, Action::VolumeReady { series, corners });
    state
}

#[test]
fn stepping_on_shows_the_next_volume_at_the_same_slice_and_view() {
    let mut state = on_the_first_echo();
    reduce(
        &mut state,
        Action::SetSlice {
            viewport: 0,
            slice: 2,
        },
    );
    let (factor, about) = (2.0, [10.0, 5.0]);
    reduce(
        &mut state,
        Action::Zoom {
            viewport: 0,
            factor,
            about,
        },
    );
    let view = state.viewports[0].view;
    let effects = step(&mut state, 1);
    let viewport = &state.viewports[0];
    assert_eq!(viewport.series, Some(id(SECOND_ECHO)));
    assert_eq!((viewport.slice, viewport.view), (2, view));
    let frames = frames_around(SECOND_ECHO, 2);
    assert!(
        frames.iter().all(|wanted| effects.contains(wanted)),
        "{effects:?}"
    );
}

#[test]
fn the_first_and_the_last_volume_are_as_far_as_it_goes() {
    let mut state = on_the_first_echo();
    assert_eq!(step(&mut state, -1), Vec::new(), "nothing before the first");
    assert_eq!(state.viewports[0].series, Some(id(FIRST_ECHO)));
    step(&mut state, 5);
    assert_eq!(
        state.viewports[0].series,
        Some(id(SECOND_ECHO)),
        "held at the last"
    );
    assert_eq!(step(&mut state, 1), Vec::new(), "nothing after the last");
    step(&mut state, -1);
    assert_eq!(state.viewports[0].series, Some(id(FIRST_ECHO)));
}

#[test]
fn a_window_set_by_hand_stays_and_an_automatic_one_is_chosen_again() {
    let mut state = on_the_first_echo();
    let wl = WindowLevel::new(321.0, 123.0);
    reduce(&mut state, Action::SetWindow { viewport: 0, wl });
    step(&mut state, 1);
    let viewport = &state.viewports[0];
    assert_eq!(
        (viewport.wl, viewport.wl_auto, viewport.wl_pending),
        (wl, false, false)
    );

    let mut state = on_the_first_echo();
    assert!(state.viewports[0].wl_auto);
    state.viewports[0].wl_pending = false;
    step(&mut state, 1);
    assert!(
        state.viewports[0].wl_pending,
        "to be chosen for the new volume"
    );
}

#[test]
fn the_scene_takes_the_new_volume_and_keeps_its_camera_its_look_and_its_cuts() {
    let mut state = first_echo_ready();
    reduce(&mut state, Action::SceneOrbit { by: [30.0, 12.0] });
    reduce(&mut state, Action::SceneLook(Look::Mip));
    reduce(&mut state, Action::CutAdd);
    let (camera, cuts) = (state.scene.camera, state.scene.cuts.clone());
    let effects = step(&mut state, 1);
    assert!(effects.contains(&build(SECOND_ECHO)), "{effects:?}");
    let scene = &state.scene;
    assert_eq!(scene.series, Some(id(SECOND_ECHO)));
    assert!(matches!(scene.status, SceneStatus::Building { .. }));
    assert_eq!(
        (scene.camera, scene.look, &scene.cuts),
        (camera, Look::Mip, &cuts)
    );
    let (series, corners) = (id(SECOND_ECHO), corners());
    reduce(&mut state, Action::VolumeReady { series, corners });
    let scene = &state.scene;
    assert_eq!(scene.status, SceneStatus::Ready);
    assert_eq!(scene.camera, camera, "not fitted afresh");
    assert!(!scene.untouched, "still the person's own camera");
}

#[test]
fn another_series_altogether_still_starts_the_scene_afresh() {
    let mut state = first_echo_ready();
    reduce(&mut state, Action::SceneLook(Look::Mip));
    select(&mut state, 0, THICK_SCAN);
    assert_eq!(state.scene.look, Look::Surface);
    assert!(state.scene.corners.is_none());
}

#[test]
fn a_series_of_one_volume_has_nowhere_to_step() {
    let mut state = on_the_first_echo();
    select(&mut state, 0, THICK_SCAN);
    assert_eq!(step(&mut state, 1), Vec::new());
    assert_eq!(state.viewports[0].series, Some(id(THICK_SCAN)));
}
