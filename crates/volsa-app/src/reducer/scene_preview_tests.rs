//! A long series is drawn as a preview while it builds, and the scene builds nothing while it
//! is not wanted.

use super::scene_tests::{build, corners, select};
use super::tests::showing;
use super::*;
use crate::scene::SceneStatus;
use crate::testing::{FINE_SCAN, THICK_SCAN, id};

fn preview_of(series: &str) -> Action {
    Action::PreviewReady {
        series: id(series),
        corners: corners(),
    }
}

#[test]
fn a_preview_is_drawn_only_while_its_series_builds() {
    let mut state = showing(THICK_SCAN);
    reduce(&mut state, preview_of(FINE_SCAN));
    assert!(!state.scene.preview, "another series' preview");
    reduce(&mut state, preview_of(THICK_SCAN));
    assert!(state.scene.preview);
    assert_eq!(state.scene.corners, Some(corners()));
    assert!(matches!(state.scene.status, SceneStatus::Building { .. }));
    let (series, corners) = (id(THICK_SCAN), corners());
    reduce(&mut state, Action::VolumeReady { series, corners });
    assert!(!state.scene.preview, "the volume itself is drawn now");
    assert_eq!(state.scene.status, SceneStatus::Ready);

    let mut failed = showing(THICK_SCAN);
    let reason = "Slice 3 could not be decoded.".to_owned();
    let series = id(THICK_SCAN);
    reduce(&mut failed, Action::VolumeFailed { series, reason });
    reduce(&mut failed, preview_of(THICK_SCAN));
    assert!(!failed.scene.preview, "a preview after the build failed");
}

#[test]
fn a_camera_moved_on_the_preview_stays_where_it_was_put() {
    let mut state = showing(THICK_SCAN);
    reduce(&mut state, preview_of(THICK_SCAN));
    let fitted = state.scene.camera;
    reduce(&mut state, Action::SceneOrbit { by: [0.3, 0.1] });
    let moved = state.scene.camera;
    assert_ne!(moved, fitted, "fitted to the series first, then moved");
    let (series, corners) = (id(THICK_SCAN), corners());
    reduce(&mut state, Action::VolumeReady { series, corners });
    assert_eq!(state.scene.camera, moved);
}

#[test]
fn nothing_is_measured_on_a_preview() {
    let mut state = showing(THICK_SCAN);
    reduce(&mut state, preview_of(THICK_SCAN));
    assert!(
        crate::planes::grid(&state).is_none(),
        "no planes through it"
    );
}

#[test]
fn no_volume_is_built_while_the_scene_is_not_wanted() {
    let mut state = showing(THICK_SCAN);
    assert_eq!(reduce(&mut state, Action::SceneWanted(false)), Vec::new());
    assert_eq!(state.scene.status, SceneStatus::Empty);
    let effects = select(&mut state, 0, FINE_SCAN);
    assert!(!effects.contains(&build(FINE_SCAN)), "{effects:?}");
    assert_eq!(state.scene.status, SceneStatus::Empty);
    let effects = reduce(&mut state, Action::SceneWanted(true));
    assert!(effects.contains(&build(FINE_SCAN)), "{effects:?}");
    assert!(matches!(state.scene.status, SceneStatus::Building { .. }));
}
