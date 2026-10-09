//! What a viewport shows, which slice, and how the picture is placed.

use voxa_core::WindowLevel;
use voxa_render::ViewTransform;

use super::tests::{frames_around, loaded, showing};
use super::*;
use crate::state::Layout;
use crate::testing::{THICK_SCAN, id};

#[test]
fn selecting_a_series_shows_its_middle_slice_fitted_and_asks_for_frames() {
    let mut state = loaded();
    reduce(&mut state, Action::SetLayout(Layout::TwoAcross));
    state.viewports[1].view = ViewTransform::fit().turned_clockwise();
    let effects = reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 1,
            series: id(THICK_SCAN),
        },
    );
    let mut expected = frames_around(THICK_SCAN, 15);
    expected.push(Effect::BuildVolume {
        series: id(THICK_SCAN),
    });
    expected.push(Effect::LoadViews {
        series: id(THICK_SCAN),
    });
    assert_eq!(
        effects, expected,
        "its frames, and its volume for the 3D scene"
    );
    let viewport = &state.viewports[1];
    assert_eq!(viewport.series, Some(id(THICK_SCAN)));
    assert_eq!((viewport.slice, viewport.view), (15, ViewTransform::fit()));
    assert!(viewport.wl_auto && viewport.wl_pending);
    assert_eq!(state.active, 1);
}

#[test]
fn selecting_a_series_that_is_not_in_the_export_changes_nothing() {
    let mut state = loaded();
    let before = state.clone();
    let effects = reduce(
        &mut state,
        Action::SelectSeries {
            viewport: 0,
            series: id("9.9.9"),
        },
    );
    assert_eq!((effects, state), (Vec::new(), before));
}

#[test]
fn scrolling_stops_at_either_end() {
    let mut state = showing(THICK_SCAN);
    let effects = reduce(
        &mut state,
        Action::Scroll {
            viewport: 0,
            delta: 3,
        },
    );
    assert_eq!(
        (state.viewports[0].slice, effects),
        (18, frames_around(THICK_SCAN, 18))
    );
    reduce(
        &mut state,
        Action::Scroll {
            viewport: 0,
            delta: 1000,
        },
    );
    assert_eq!(state.viewports[0].slice, 29);
    let at_the_end = reduce(
        &mut state,
        Action::Scroll {
            viewport: 0,
            delta: 1,
        },
    );
    assert_eq!(
        (state.viewports[0].slice, at_the_end),
        (29, Vec::new()),
        "nothing new to fetch"
    );
    reduce(
        &mut state,
        Action::Scroll {
            viewport: 0,
            delta: i32::MIN,
        },
    );
    assert_eq!(state.viewports[0].slice, 0);
}

#[test]
fn going_to_a_slice_stops_at_the_last() {
    let mut state = showing(THICK_SCAN);
    reduce(
        &mut state,
        Action::SetSlice {
            viewport: 0,
            slice: 7,
        },
    );
    assert_eq!(state.viewports[0].slice, 7);
    reduce(
        &mut state,
        Action::SetSlice {
            viewport: 0,
            slice: usize::MAX,
        },
    );
    assert_eq!(state.viewports[0].slice, 29);
}

#[test]
fn four_quarter_turns_come_back_to_the_view_there_was() {
    let mut state = showing(THICK_SCAN);
    reduce(
        &mut state,
        Action::Flip {
            viewport: 0,
            horizontal: true,
        },
    );
    let before = state.viewports[0].view;
    for turn in 1..=4 {
        reduce(&mut state, Action::Rotate { viewport: 0 });
        assert_eq!(state.viewports[0].view == before, turn == 4, "after {turn}");
    }
}

#[test]
fn flips_mirror_the_picture_as_it_is_seen() {
    let mut state = showing(THICK_SCAN);
    reduce(&mut state, Action::Rotate { viewport: 0 });
    reduce(
        &mut state,
        Action::Flip {
            viewport: 0,
            horizontal: true,
        },
    );
    let expected = ViewTransform::fit().turned_clockwise().flipped(true);
    assert_eq!(state.viewports[0].view, expected);
}

#[test]
fn zoom_pan_and_fit_change_only_the_view() {
    let mut state = showing(THICK_SCAN);
    reduce(
        &mut state,
        Action::Flip {
            viewport: 0,
            horizontal: false,
        },
    );
    reduce(
        &mut state,
        Action::Zoom {
            viewport: 0,
            factor: 2.0,
            about: [10.0, 0.0],
        },
    );
    reduce(
        &mut state,
        Action::Pan {
            viewport: 0,
            by: [5.0, -3.0],
        },
    );
    let view = state.viewports[0].view;
    assert_eq!((view.zoom, view.pan), (2.0, [-5.0, -3.0]));
    reduce(&mut state, Action::Fit { viewport: 0 });
    let view = state.viewports[0].view;
    assert_eq!((view.zoom, view.pan), (1.0, [0.0, 0.0]));
    assert!(view.flip_v, "fitting keeps the picture the way up it was");
}

#[test]
fn a_viewport_that_does_not_exist_is_ignored() {
    let mut state = showing(THICK_SCAN);
    let before = state.clone();
    let wl = WindowLevel::new(1.0, 2.0);
    let actions = [
        Action::SelectSeries {
            viewport: 4,
            series: id(THICK_SCAN),
        },
        Action::Scroll {
            viewport: 9,
            delta: 1,
        },
        Action::SetSlice {
            viewport: usize::MAX,
            slice: 0,
        },
        Action::Zoom {
            viewport: 4,
            factor: 2.0,
            about: [0.0, 0.0],
        },
        Action::Pan {
            viewport: 4,
            by: [1.0, 1.0],
        },
        Action::Flip {
            viewport: 4,
            horizontal: true,
        },
        Action::Rotate { viewport: 4 },
        Action::Fit { viewport: 4 },
        Action::AdjustWindow {
            viewport: 4,
            d_center: 1.0,
            d_width: 1.0,
        },
        Action::SetWindow { viewport: 4, wl },
        Action::ApplyPreset {
            viewport: 4,
            preset: 0,
        },
        Action::CineToggle { viewport: 4 },
        Action::CineSet {
            viewport: 4,
            fps: 5.0,
            looping: true,
        },
        Action::CineTick { viewport: 4 },
    ];
    for action in actions {
        assert_eq!(reduce(&mut state, action.clone()), Vec::new(), "{action:?}");
        assert_eq!(state, before, "{action:?}");
    }
}

#[test]
fn a_viewport_showing_nothing_ignores_movement() {
    let mut state = loaded();
    let before = state.clone();
    for action in [
        Action::Scroll {
            viewport: 0,
            delta: 1,
        },
        Action::CineTick { viewport: 0 },
    ] {
        assert_eq!(reduce(&mut state, action), Vec::new());
    }
    assert_eq!(state, before);
}
