//! The 3D scene in the state: which series it shows, its camera, its cuts.

use voxa_core::{Patient, Point};
use voxa_render::{Camera, Look, NamedView};

use super::tests::{frames_around, loaded, showing};
use super::*;
use crate::pictures::Pictures;
use crate::scene::{Scene, SceneStatus};
use crate::state::Layout;
use crate::testing::{FINE_SCAN, LOCALISER, SINGLE, THICK_SCAN, id, with_odd_series};

pub(super) fn select(state: &mut AppState, viewport: usize, series: &str) -> Vec<Effect> {
    let series = id(series);
    reduce(state, Action::SelectSeries { viewport, series })
}

/// What starting the scene afresh for a series asks for: its volume, and the views saved
/// of it.
pub(super) fn started(series: &str) -> Vec<Effect> {
    let series = id(series);
    let views = Effect::LoadViews {
        series: series.clone(),
    };
    vec![build(&series.0), views, Effect::LoadAnnotations { series }]
}

pub(super) fn build(series: &str) -> Effect {
    Effect::BuildVolume { series: id(series) }
}

/// The corners of a box 40 by 60 by 20 mm about a point away from the origin.
pub(super) fn corners() -> [Point<Patient>; 8] {
    let end = |far: bool, half: f64| if far { half } else { -half };
    std::array::from_fn(|i| {
        let [x, y, z] = [
            end(i & 1 != 0, 20.0),
            end(i & 2 != 0, 30.0),
            end(i & 4 != 0, 10.0),
        ];
        Point::new(x + 5.0, y - 7.0, z + 50.0)
    })
}

/// A state whose scene shows the 30-slice series, with its volume built.
pub(super) fn ready() -> AppState {
    let mut state = showing(THICK_SCAN);
    let (series, corners) = (id(THICK_SCAN), corners());
    reduce(&mut state, Action::VolumeReady { series, corners });
    state
}

#[test]
fn a_fresh_scene_is_empty_and_the_window_shows_the_slices() {
    let state = AppState::default();
    assert_eq!(
        state.pictures,
        Pictures::Slices,
        "a newcomer measures on a slice"
    );
    assert_eq!(state.scene, Scene::default());
    assert_eq!(
        (state.scene.series.clone(), &state.scene.status),
        (None, &SceneStatus::Empty)
    );
    assert!(state.scene.untouched && state.scene.cuts.is_empty() && !state.scene.slice_shown);
    assert_eq!((state.scene.look, state.scene.halved), (Look::Surface, 0));
}

#[test]
fn selecting_a_series_starts_building_its_volume() {
    let mut state = loaded();
    let effects = select(&mut state, 0, THICK_SCAN);
    assert_eq!(
        effects,
        [frames_around(THICK_SCAN, 15), started(THICK_SCAN)].concat()
    );
    assert_eq!(state.scene.series, Some(id(THICK_SCAN)));
    assert_eq!(
        state.scene.status,
        SceneStatus::Building { done: 0, of: 30 }
    );
}

#[test]
fn the_best_series_opened_when_indexing_ends_is_built_too() {
    let mut state = loaded();
    let effects = reduce(&mut state, Action::IndexDone);
    assert!(effects.ends_with(&started(FINE_SCAN)), "{effects:?}");
    assert_eq!(
        state.scene.status,
        SceneStatus::Building { done: 0, of: 64 }
    );
}

#[test]
fn a_series_chosen_while_indexing_is_built_again_when_indexing_ends() {
    // It may have gained slices since.
    let mut state = showing(THICK_SCAN);
    reduce(&mut state, Action::SceneOrbit { by: [30.0, 0.0] });
    let effects = reduce(&mut state, Action::IndexDone);
    assert!(effects.contains(&build(THICK_SCAN)), "{effects:?}");
    assert_eq!(
        state.scene.status,
        SceneStatus::Building { done: 0, of: 30 }
    );
}

#[test]
fn the_scene_follows_the_active_viewport() {
    let mut state = showing(THICK_SCAN);
    reduce(&mut state, Action::SetLayout(Layout::TwoAcross));
    assert!(select(&mut state, 1, FINE_SCAN).ends_with(&started(FINE_SCAN)));
    assert_eq!(state.scene.series, Some(id(FINE_SCAN)));
    assert_eq!(
        reduce(&mut state, Action::SetActive(0)),
        started(THICK_SCAN)
    );
    assert_eq!(state.scene.series, Some(id(THICK_SCAN)));
    // The second viewport goes with a single layout; the scene stays with the first.
    reduce(&mut state, Action::SetActive(1));
    assert_eq!(
        reduce(&mut state, Action::SetLayout(Layout::One)),
        started(THICK_SCAN)
    );
}

#[test]
fn scrolling_and_other_changes_to_the_same_series_build_nothing() {
    let mut state = ready();
    let effects = reduce(
        &mut state,
        Action::Scroll {
            viewport: 0,
            delta: 3,
        },
    );
    assert_eq!(effects, frames_around(THICK_SCAN, 18));
    assert_eq!(reduce(&mut state, Action::SetActive(0)), []);
    assert_eq!(state.scene.status, SceneStatus::Ready);
}

#[test]
fn a_new_series_starts_the_scene_afresh() {
    let mut state = ready();
    reduce(&mut state, Action::SceneLook(Look::Mip));
    reduce(&mut state, Action::CutAdd);
    reduce(&mut state, Action::SceneUploaded { halved: 2 });
    reduce(
        &mut state,
        Action::SceneResized {
            viewport: [640.0, 480.0],
        },
    );
    select(&mut state, 0, FINE_SCAN);
    let fresh = Scene {
        series: Some(id(FINE_SCAN)),
        status: SceneStatus::Building { done: 0, of: 64 },
        viewport: [640.0, 480.0],
        ..Scene::default()
    };
    assert_eq!(
        state.scene, fresh,
        "everything but the size of the picture starts again"
    );
}

#[test]
fn a_set_of_locating_images_says_why_it_has_no_3d_view() {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(with_odd_series()));
    let effects = select(&mut state, 0, LOCALISER);
    assert_eq!(
        effects,
        frames_around(LOCALISER, 1),
        "the 2D viewport carries on"
    );
    // Why, said once each, and then what is done about it, said once.
    let expected = vec![
        "This series cannot be shown in 3D.".to_owned(),
        "The slices do not all face the same way.".to_owned(),
        "It is shown as a plain stack.".to_owned(),
    ];
    assert_eq!(state.scene.status, SceneStatus::Unavailable(expected));
    assert_eq!(state.scene.series, Some(id(LOCALISER)));
}

#[test]
fn a_single_slice_says_that_3d_needs_more() {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(with_odd_series()));
    assert_eq!(select(&mut state, 0, SINGLE), frames_around(SINGLE, 0));
    let SceneStatus::Unavailable(lines) = &state.scene.status else {
        panic!("{:?}", state.scene.status);
    };
    let needs_two = "A 3D view needs at least two slices; this series has 1.";
    assert_eq!(lines.first().map(String::as_str), Some(needs_two));
    let stack = "It is shown as a plain stack.";
    assert_eq!(lines.last().map(String::as_str), Some(stack));
    let said_twice = lines
        .iter()
        .filter(|line| line.contains("plain stack"))
        .count();
    assert_eq!(said_twice, 1, "{lines:?}");
}

#[test]
fn a_finished_volume_is_shown_fitted_from_the_front() {
    let mut state = showing(THICK_SCAN);
    let (series, done) = (id(THICK_SCAN), 16);
    reduce(
        &mut state,
        Action::VolumeProgress {
            series,
            done,
            of: 30,
        },
    );
    assert_eq!(
        state.scene.status,
        SceneStatus::Building { done: 16, of: 30 }
    );
    let state = ready();
    assert_eq!(state.scene.status, SceneStatus::Ready);
    assert_eq!(state.scene.corners, Some(corners()));
    let fitted = Camera::fitted(NamedView::Anterior, &corners(), Scene::FIRST_VIEWPORT);
    assert_eq!(state.scene.camera, fitted);
    assert!(state.scene.untouched);
}

#[test]
fn reports_about_a_volume_the_scene_no_longer_shows_are_ignored() {
    let mut state = showing(THICK_SCAN);
    let before = state.clone();
    let other = || id(FINE_SCAN);
    reduce(
        &mut state,
        Action::VolumeReady {
            series: other(),
            corners: corners(),
        },
    );
    reduce(
        &mut state,
        Action::VolumeProgress {
            series: other(),
            done: 8,
            of: 64,
        },
    );
    let reason = "it broke".to_owned();
    reduce(
        &mut state,
        Action::VolumeFailed {
            series: other(),
            reason,
        },
    );
    assert_eq!(state, before);
    // Nor does a late report change a scene that is already finished.
    let mut state = ready();
    let late = Action::VolumeFailed {
        series: id(THICK_SCAN),
        reason: "late".to_owned(),
    };
    reduce(&mut state, late);
    assert_eq!(state.scene.status, SceneStatus::Ready);
}

#[test]
fn a_volume_that_could_not_be_built_says_why() {
    let mut state = showing(THICK_SCAN);
    let reason = "Slice 7 could not be decoded.".to_owned();
    reduce(
        &mut state,
        Action::VolumeFailed {
            series: id(THICK_SCAN),
            reason: reason.clone(),
        },
    );
    assert_eq!(state.scene.status, SceneStatus::Unavailable(vec![reason]));
}
