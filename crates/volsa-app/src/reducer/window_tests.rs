//! The window and level, and film-like playback.

use volsa_core::WindowLevel;

use super::tests::{frames_around, showing, windows};
use super::*;
use crate::state::Cine;
use crate::testing::{SCOUT, THICK_SCAN, id};

#[test]
fn dragging_the_window_never_takes_the_width_below_one_and_ends_automatic_windowing() {
    let mut state = showing(THICK_SCAN);
    reduce(
        &mut state,
        Action::SetWindow {
            viewport: 0,
            wl: WindowLevel::new(40.0, 80.0),
        },
    );
    reduce(
        &mut state,
        Action::AdjustWindow {
            viewport: 0,
            d_center: 5.0,
            d_width: -500.0,
        },
    );
    let viewport = &state.viewports[0];
    assert_eq!((viewport.wl.center, viewport.wl.width), (45.0, 1.0));
    assert!(!viewport.wl_auto && !viewport.wl_pending);
}

#[test]
fn the_first_frame_of_the_slice_shown_sets_the_window_once() {
    let mut state = showing(THICK_SCAN);
    let ready = |slice, center| Action::FrameReady {
        series: id(THICK_SCAN),
        slice,
        windows: windows(center),
    };
    reduce(&mut state, ready(3, 900.0));
    assert!(
        state.viewports[0].wl_pending,
        "slice 3 is not the one shown"
    );
    reduce(&mut state, ready(15, 300.0));
    assert_eq!(state.viewports[0].wl, WindowLevel::new(300.0, 100.0));
    assert!(state.viewports[0].wl_auto && !state.viewports[0].wl_pending);
    reduce(
        &mut state,
        Action::Scroll {
            viewport: 0,
            delta: -12,
        },
    );
    reduce(&mut state, ready(16, 700.0));
    assert_eq!(
        state.viewports[0].wl.center, 300.0,
        "one window for the series, not one a slice"
    );
}

#[test]
fn a_window_set_by_hand_is_not_replaced_when_the_frame_arrives() {
    let mut state = showing(THICK_SCAN);
    reduce(
        &mut state,
        Action::SetWindow {
            viewport: 0,
            wl: WindowLevel::new(40.0, 80.0),
        },
    );
    let ready = Action::FrameReady {
        series: id(THICK_SCAN),
        slice: 15,
        windows: windows(300.0),
    };
    reduce(&mut state, ready);
    assert_eq!(state.viewports[0].wl, WindowLevel::new(40.0, 80.0));
}

#[test]
fn scrolling_to_a_slice_already_decoded_settles_a_window_still_waiting() {
    let mut state = showing(THICK_SCAN);
    let ready = Action::FrameReady {
        series: id(THICK_SCAN),
        slice: 16,
        windows: windows(250.0),
    };
    reduce(&mut state, ready);
    reduce(
        &mut state,
        Action::Scroll {
            viewport: 0,
            delta: 1,
        },
    );
    assert_eq!(state.viewports[0].wl.center, 250.0);
    assert!(!state.viewports[0].wl_pending);
}

#[test]
fn a_preset_uses_the_window_worked_out_for_the_slice_shown() {
    let mut state = showing(THICK_SCAN);
    let ready = Action::FrameReady {
        series: id(THICK_SCAN),
        slice: 15,
        windows: windows(300.0),
    };
    reduce(&mut state, ready);
    reduce(
        &mut state,
        Action::ApplyPreset {
            viewport: 0,
            preset: 2,
        },
    );
    assert_eq!(state.viewports[0].wl, WindowLevel::new(300.0, 50.0));
    assert!(!state.viewports[0].wl_auto);
    reduce(
        &mut state,
        Action::ApplyPreset {
            viewport: 0,
            preset: 0,
        },
    );
    assert_eq!(state.viewports[0].wl, WindowLevel::new(300.0, 100.0));
    assert!(
        state.viewports[0].wl_auto,
        "the first preset is the automatic one"
    );
    let before = state.clone();
    reduce(
        &mut state,
        Action::ApplyPreset {
            viewport: 0,
            preset: 99,
        },
    );
    assert_eq!(state, before, "there is no such preset");
}

#[test]
fn a_preset_asked_for_before_the_slice_is_decoded_waits_only_if_it_is_the_automatic_one() {
    let mut state = showing(THICK_SCAN);
    reduce(
        &mut state,
        Action::SetWindow {
            viewport: 0,
            wl: WindowLevel::new(40.0, 80.0),
        },
    );
    reduce(
        &mut state,
        Action::ApplyPreset {
            viewport: 0,
            preset: 2,
        },
    );
    assert_eq!(state.viewports[0].wl, WindowLevel::new(40.0, 80.0));
    reduce(
        &mut state,
        Action::ApplyPreset {
            viewport: 0,
            preset: 0,
        },
    );
    assert!(state.viewports[0].wl_auto && state.viewports[0].wl_pending);
}

#[test]
fn playback_wraps_at_the_last_slice_when_looping_and_stops_when_not() {
    let mut state = showing(SCOUT);
    reduce(&mut state, Action::CineToggle { viewport: 0 });
    assert_eq!(
        state.viewports[0].cine,
        Some(Cine::new(Cine::DEFAULT_FPS, true, true))
    );
    let effects = reduce(&mut state, Action::CineTick { viewport: 0 });
    assert_eq!(
        (state.viewports[0].slice, effects),
        (2, frames_around(SCOUT, 2))
    );
    reduce(&mut state, Action::CineTick { viewport: 0 });
    assert_eq!(state.viewports[0].slice, 0, "wrapped");

    reduce(
        &mut state,
        Action::CineSet {
            viewport: 0,
            fps: 500.0,
            looping: false,
        },
    );
    assert_eq!(state.viewports[0].cine, Some(Cine::new(60.0, false, true)));
    reduce(
        &mut state,
        Action::SetSlice {
            viewport: 0,
            slice: 2,
        },
    );
    reduce(&mut state, Action::CineTick { viewport: 0 });
    let viewport = &state.viewports[0];
    assert_eq!(
        (viewport.slice, viewport.cine.map(|cine| cine.playing)),
        (2, Some(false))
    );
}

#[test]
fn a_tick_does_nothing_while_playback_is_paused_or_was_never_started() {
    let mut state = showing(THICK_SCAN);
    reduce(&mut state, Action::CineTick { viewport: 0 });
    assert_eq!(state.viewports[0].slice, 15);
    reduce(&mut state, Action::CineToggle { viewport: 0 });
    reduce(&mut state, Action::CineToggle { viewport: 0 });
    reduce(&mut state, Action::CineTick { viewport: 0 });
    assert_eq!(state.viewports[0].slice, 15);
}
