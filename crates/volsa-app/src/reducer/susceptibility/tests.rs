//! A susceptibility map in the state: asked for only of a phase series whose volume is built,
//! shown in place of the phase with its own window, given up as the scene moves to another
//! series, and left for the phase, which is built again the usual way.

use super::*;
use crate::annotations::Shape;
use crate::labelling::LabellingAction;
use crate::reducer::reduce;
use crate::state::NoticeKind;
use crate::testing::{FINE_SCAN, THICK_SCAN, id, thick_scan_ready};

/// What a phase series of GE's says of itself.
const PHASE: &str = "FILT_PHA: 3D Ax eSWAN";

/// The 30-slice series named as phase, with the echo time and field strength its files would
/// give, shown with its volume ready, or not yet named or given them, as `named` and `facts`
/// say.
fn phase_ready(named: bool, facts: bool) -> AppState {
    let mut state = thick_scan_ready();
    let index = state.index.as_mut().expect("indexed");
    let listed = index
        .series
        .iter_mut()
        .find(|listed| listed.info.id == id(THICK_SCAN));
    let info = &mut listed.expect("listed").info;
    if named {
        info.description = PHASE.to_owned();
    }
    if facts {
        info.scan.echo_ms = Some(25.0);
        info.scan.field_t = Some(3.0);
    }
    state
}

fn make(state: &mut AppState) -> Vec<Effect> {
    reduce(
        state,
        Action::Labelling(LabellingAction::MakeSusceptibility),
    )
}

fn made(effects: &[Effect]) -> bool {
    effects
        .iter()
        .any(|effect| matches!(effect, Effect::MakeSusceptibility { .. }))
}

/// The map of the 30-slice series, made and shown.
fn map_shown() -> AppState {
    let mut state = phase_ready(true, true);
    make(&mut state);
    let corners = state.scene.corners.expect("built");
    let series = id(THICK_SCAN);
    reduce(&mut state, Action::VolumeReady { series, corners });
    state
}

/// The text of the notice with `key`, if there is one.
fn said(state: &AppState, key: &str) -> Option<String> {
    let notice = state.notices.iter().find(|notice| notice.key == Some(key));
    notice.map(|notice| notice.text.clone())
}

#[test]
fn the_button_asks_for_a_map_only_of_a_phase_series_with_its_volume_built() {
    let mut state = phase_ready(true, true);
    let effects = make(&mut state);
    let facts = volsa_segment::PhaseFacts {
        echo_ms: Some(25.0),
        field_t: Some(3.0),
    };
    let asked = Effect::MakeSusceptibility {
        series: id(THICK_SCAN),
        facts,
    };
    assert!(effects.contains(&asked), "{effects:?}");
    assert_eq!(state.scene.map, MapState::Making);
    assert_eq!(
        crate::susceptibility::can_make_map(&state),
        Err("A susceptibility map is being worked out. Wait for it to finish."),
        "not twice at once"
    );
    assert!(!made(&make(&mut state)));
}

#[test]
fn a_map_is_not_asked_for_and_the_reason_is_said_when_it_cannot_be_made() {
    // Not named as phase.
    let mut state = phase_ready(false, true);
    assert!(!made(&make(&mut state)));
    assert_eq!(state.scene.map, MapState::Off);
    let why = said(&state, KEY).expect("why is said");
    assert!(why.contains("not named as phase"), "{why}");
    // Named, but its files do not give what the map needs.
    let mut state = phase_ready(true, false);
    assert!(!made(&make(&mut state)));
    let why = said(&state, KEY).expect("why is said");
    assert!(why.contains("echo time and field strength"), "{why}");
    // Named, but its volume not yet built.
    let mut state = phase_ready(true, true);
    state.scene.status = crate::scene::SceneStatus::Building { done: 3, of: 30 };
    assert!(!made(&make(&mut state)));
    let why = said(&state, KEY).expect("why is said");
    assert!(why.contains("3D view"), "{why}");
}

#[test]
fn a_map_ready_is_shown_through_its_own_window_in_place_of_the_phase() {
    let mut state = phase_ready(true, true);
    let series = id(THICK_SCAN);
    let slice = state.viewports[0].slice;
    let windows = [volsa_core::WindowLevel::new(0.5, 1.0); 3];
    state.windows.insert((series.clone(), slice), windows);
    make(&mut state);
    let corners = state.scene.corners.expect("built");
    let effects = reduce(
        &mut state,
        Action::VolumeReady {
            series: series.clone(),
            corners,
        },
    );
    assert_eq!(state.scene.map, MapState::Shown);
    assert!(crate::susceptibility::shown(&state, &series));
    let viewport = &state.viewports[0];
    assert_eq!(viewport.wl, crate::susceptibility::window());
    assert_eq!((viewport.wl.center, viewport.wl.width), (0.0, 300.0));
    assert!(!viewport.wl_pending && !viewport.wl_auto);
    assert!(
        !state.windows.contains_key(&(series.clone(), slice)),
        "the phase's windows are not the map's"
    );
    assert!(
        effects.contains(&Effect::RequestFrames {
            series: series.clone(),
            around: slice
        }),
        "the slice is said again, of the map: {effects:?}"
    );
    let kept = |effect: &Effect| matches!(effect, Effect::FindKept { .. });
    assert!(
        !effects.iter().any(kept),
        "the map is not looked up as if it were the series' own volume"
    );
    // A slice said of the map afterwards does not change the window.
    let map_windows = [volsa_core::WindowLevel::new(10.0, 40.0); 3];
    reduce(
        &mut state,
        Action::FrameReady {
            series,
            slice,
            windows: map_windows,
        },
    );
    assert_eq!(state.viewports[0].wl, crate::susceptibility::window());
}

#[test]
fn a_map_that_could_not_be_made_leaves_the_phase_shown_and_says_why() {
    let mut state = phase_ready(true, true);
    make(&mut state);
    let reason = "No head was found in this phase. Choose the phase series of an SWI scan.";
    let failed = Action::VolumeFailed {
        series: id(THICK_SCAN),
        reason: reason.to_owned(),
    };
    reduce(&mut state, failed);
    assert_eq!(state.scene.map, MapState::Off);
    assert_eq!(state.scene.status, crate::scene::SceneStatus::Ready);
    assert_eq!(said(&state, KEY).as_deref(), Some(reason));
    let notice = state.notices.iter().find(|notice| notice.key == Some(KEY));
    assert_eq!(notice.map(|notice| notice.kind), Some(NoticeKind::Warning));
}

#[test]
fn opening_another_series_gives_up_the_map_and_the_state_no_longer_claims_one() {
    let mut state = map_shown();
    let effects = reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 0,
            series: id(FINE_SCAN),
        },
    );
    assert_eq!(state.scene.map, MapState::Off);
    assert_eq!(
        state.scene.look,
        volsa_render::Look::Surface,
        "the look from before"
    );
    assert!(!crate::susceptibility::shown(&state, &id(THICK_SCAN)));
    assert!(effects.contains(&Effect::DropSusceptibility), "{effects:?}");
    // And one being made is given up too.
    let mut state = phase_ready(true, true);
    make(&mut state);
    let effects = reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 0,
            series: id(FINE_SCAN),
        },
    );
    assert_eq!(state.scene.map, MapState::Off);
    assert!(effects.contains(&Effect::DropSusceptibility), "{effects:?}");
    // What it reports late is not taken for the new series' volume.
    let corners = state.scene.corners;
    let late = Action::VolumeReady {
        series: id(THICK_SCAN),
        corners: corners.unwrap_or([volsa_core::Point::new(0.0, 0.0, 0.0); 8]),
    };
    reduce(&mut state, late);
    assert_eq!(state.scene.map, MapState::Off);
}

#[test]
fn opening_another_export_leaves_no_map_claimed() {
    let mut state = map_shown();
    reduce(
        &mut state,
        Action::OpenPath(std::path::PathBuf::from("other")),
    );
    assert_eq!(state.scene.map, MapState::Off);
}

#[test]
fn back_to_the_phase_asks_for_its_volume_built_the_usual_way_and_keeps_the_view() {
    let mut state = map_shown();
    assert_eq!(
        state.scene.look,
        volsa_render::Look::Mip,
        "brightest values in 3D"
    );
    reduce(&mut state, Action::SceneOrbit { by: [30.0, 10.0] });
    let camera = state.scene.camera;
    let effects = reduce(&mut state, Action::Labelling(LabellingAction::BackToPhase));
    let series = id(THICK_SCAN);
    assert_eq!(state.scene.map, MapState::Off);
    assert!(
        effects.contains(&Effect::BuildVolume {
            series: series.clone()
        }),
        "{effects:?}"
    );
    assert!(effects.contains(&Effect::DropSusceptibility), "{effects:?}");
    let reread = |effect: &Effect| matches!(effect, Effect::LoadAnnotations { .. });
    assert!(!effects.iter().any(reread), "the measurements in hand stay");
    assert!(matches!(
        state.scene.status,
        crate::scene::SceneStatus::Building { .. }
    ));
    assert_eq!(state.scene.camera, camera, "seen as it was");
    assert_eq!(
        state.scene.look,
        volsa_render::Look::Surface,
        "in the look from before"
    );
    let viewport = &state.viewports[0];
    assert!(
        viewport.wl_pending && viewport.wl_auto,
        "a window of the phase's own is chosen again"
    );
    // Nothing to go back from now.
    let again = reduce(&mut state, Action::Labelling(LabellingAction::BackToPhase));
    assert!(again.is_empty(), "{again:?}");
}

#[test]
fn a_region_is_not_measured_on_a_map() {
    let mut state = map_shown();
    let before = state.annotations.items.len();
    let region = Shape::Region {
        centre: [0.0; 3],
        across: [1.0, 0.0, 0.0],
        down: [0.0, 1.0, 0.0],
        radii_mm: [4.0, 2.0],
        stats: None,
    };
    reduce(&mut state, Action::AnnotationAdd(region));
    assert_eq!(state.annotations.items.len(), before);
    let ruler = Shape::Ruler {
        a: [0.0; 3],
        b: [5.0, 0.0, 0.0],
    };
    reduce(&mut state, Action::AnnotationAdd(ruler));
    assert_eq!(state.annotations.items.len(), before + 1, "a length is");
}
