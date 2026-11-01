//! Stack viewports of the scene's study follow the planes' position, and take it with them
//! when they are scrolled.

use volsa_core::{Affine, Patient, Point, Voxel};
use volsa_dicom::Index;
use volsa_dicom::testkit::{SliceOptions, classic_slice};
use volsa_volume::position_on_slice;

use super::scene_tests::select;
use super::tests::frames_around;
use super::*;
use crate::planes::position_in_viewport;
use crate::state::Layout;
use crate::testing::{FINE_SCAN, THICK_SCAN, id, indexed, slice_options, three_series_files};

/// Ten slices of another study.
const ELSEWHERE: &str = "1.2.9.1";
/// Five slices of the same study whose files do not say where they lie.
const UNPLACED: &str = "1.2.3.7";

fn index() -> Index {
    let elsewhere = (0..10).map(|place| {
        let options = SliceOptions {
            study_uid: "1.2.9".to_owned(),
            ..slice_options(ELSEWHERE, 1, place, 4)
        };
        (format!("T1/I{place:04}.dcm"), classic_slice(&options))
    });
    let unplaced = (0..5).map(|place| {
        let options = SliceOptions {
            position: None,
            ..slice_options(UNPLACED, 7, place, 4)
        };
        (format!("S7/I{place:04}.dcm"), classic_slice(&options))
    });
    let files = [
        three_series_files(),
        elsewhere.collect(),
        unplaced.collect(),
    ];
    indexed(files.concat())
}

fn placement(state: &AppState, series: &str) -> Affine<Voxel, Patient> {
    let info = &state.series(&id(series)).expect("listed").info;
    info.voxel_to_patient.expect("it has geometry")
}

/// The thick scan's volume is ready in the scene.
fn made_ready(state: &mut AppState, series: &str, slices: f64) -> Vec<Effect> {
    let info = &state.series(&id(series)).expect("listed").info;
    let (cols, rows) = (f64::from(info.cols), f64::from(info.rows));
    let placement = placement(state, series);
    let end = |i: usize, bit: usize, count: f64| if i & bit == 0 { -0.5 } else { count - 0.5 };
    let corners = std::array::from_fn(|i| {
        placement.apply(Point::new(
            end(i, 1, cols),
            end(i, 2, rows),
            end(i, 4, slices),
        ))
    });
    let series = id(series);
    reduce(state, Action::VolumeReady { series, corners })
}

/// Four viewports: the 30-slice series, which is in the scene with its volume ready; the
/// 64-slice series of the same study; a series of another study; and one of the same study
/// whose files do not say where it lies.
fn four() -> AppState {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(index()));
    reduce(&mut state, Action::SetLayout(Layout::TwoByTwo));
    for (viewport, series) in [
        (3, UNPLACED),
        (2, ELSEWHERE),
        (1, FINE_SCAN),
        (0, THICK_SCAN),
    ] {
        select(&mut state, viewport, series);
    }
    assert_eq!(state.scene.series, Some(id(THICK_SCAN)));
    made_ready(&mut state, THICK_SCAN, 30.0);
    state
}

fn slices(state: &AppState) -> [usize; 4] {
    std::array::from_fn(|at| state.viewports[at].slice)
}

fn position(state: &AppState) -> Point<Patient> {
    state.planes.position.expect("there is a position")
}

#[test]
fn the_export_has_two_studies_and_a_series_that_is_not_placed() {
    let state = four();
    let index = state.index.as_ref().expect("indexed");
    assert_eq!(index.studies.len(), 2);
    let unplaced = &state.series(&id(UNPLACED)).expect("listed").info;
    assert_eq!(unplaced.voxel_to_patient, None);
}

#[test]
fn once_the_volume_is_ready_the_series_of_its_study_come_to_the_position() {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(index()));
    reduce(&mut state, Action::SetLayout(Layout::TwoByTwo));
    for (viewport, series) in [
        (3, UNPLACED),
        (2, ELSEWHERE),
        (1, FINE_SCAN),
        (0, THICK_SCAN),
    ] {
        select(&mut state, viewport, series);
    }
    assert_eq!(slices(&state), [15, 32, 5, 2], "each at its middle slice");
    let effects = made_ready(&mut state, THICK_SCAN, 30.0);
    // Both series' slices are 5 mm apart from the same place, so slice 15 is slice 15.
    assert_eq!(slices(&state), [15, 15, 5, 2]);
    // The volume that is ready asks whether a result is kept for it; then the frames.
    let find = Effect::FindKept {
        series: id(THICK_SCAN),
    };
    let asked = [vec![find], frames_around(FINE_SCAN, 15)];
    assert_eq!(effects, asked.concat());
}

#[test]
fn moving_the_position_sends_the_linked_viewports_to_the_slice_nearest_it() {
    let mut state = four();
    let effects = reduce(&mut state, Action::SetPosition(Point::new(1.0, 1.0, 101.0)));
    assert_eq!(slices(&state), [20, 20, 5, 2]);
    let asked = [frames_around(THICK_SCAN, 20), frames_around(FINE_SCAN, 20)];
    assert_eq!(effects, asked.concat(), "frames for each that moved");
    let effects = reduce(&mut state, Action::SetPosition(Point::new(1.5, 1.0, 101.0)));
    assert_eq!(effects, [], "a move within the slice asks for nothing");
}

#[test]
fn a_viewport_out_of_sight_is_left_where_it_is() {
    let mut state = four();
    reduce(&mut state, Action::SetLayout(Layout::One));
    let effects = reduce(&mut state, Action::SetPosition(Point::new(1.0, 1.0, 101.0)));
    assert_eq!(slices(&state), [20, 15, 5, 2]);
    assert_eq!(effects, frames_around(THICK_SCAN, 20));
    // Brought back into sight, it comes to the position.
    let effects = reduce(&mut state, Action::SetLayout(Layout::TwoByTwo));
    assert_eq!(slices(&state), [20, 20, 5, 2]);
    assert_eq!(effects, frames_around(FINE_SCAN, 20));
}

#[test]
fn with_the_link_off_nothing_follows_and_turning_it_on_brings_them_at_once() {
    let mut state = four();
    assert!(state.planes.linked, "linked from the start");
    assert_eq!(reduce(&mut state, Action::SetLinked(false)), []);
    assert!(!state.planes.linked);
    let effects = reduce(&mut state, Action::SetPosition(Point::new(1.0, 1.0, 101.0)));
    assert_eq!((slices(&state), effects), ([15, 15, 5, 2], vec![]));
    let was = position(&state);
    reduce(
        &mut state,
        Action::Scroll {
            viewport: 1,
            delta: 3,
        },
    );
    assert_eq!(slices(&state), [15, 18, 5, 2]);
    assert_eq!(
        position(&state),
        was,
        "and a scroll leaves the position alone"
    );
    let effects = reduce(&mut state, Action::SetLinked(true));
    assert_eq!(slices(&state), [20, 20, 5, 2]);
    let asked = [frames_around(THICK_SCAN, 20), frames_around(FINE_SCAN, 20)];
    assert_eq!(effects, asked.concat());
}

#[test]
fn scrolling_a_linked_viewport_takes_the_position_onto_the_slice_it_shows() {
    let mut state = four();
    let ear = placement(&state, FINE_SCAN);
    let before = position(&state);
    let (on_before, away) = position_on_slice(&ear, 15, before).expect("found");
    assert!(away.abs() < 1.0e-9);
    let effects = reduce(
        &mut state,
        Action::Scroll {
            viewport: 1,
            delta: 2,
        },
    );
    let after = position(&state);
    let (on_after, away) = position_on_slice(&ear, 17, after).expect("found");
    assert!(away.abs() < 1.0e-9, "on the slice now shown: {away}");
    let kept = on_before.into_iter().zip(on_after);
    assert!(
        kept.clone().all(|(was, is)| (was - is).abs() < 1.0e-9),
        "{kept:?}"
    );
    assert!(
        (after.z - before.z - 10.0).abs() < 1.0e-9,
        "two slices 5 mm apart"
    );
    assert_eq!(
        slices(&state),
        [17, 17, 5, 2],
        "and the other series comes too"
    );
    let asked = [frames_around(FINE_SCAN, 17), frames_around(THICK_SCAN, 17)];
    assert_eq!(effects, asked.concat());
}

#[test]
fn a_viewport_scrolled_past_the_volume_stays_where_it_was_scrolled_to() {
    let mut state = four();
    // The 64-slice series reaches far beyond the 30 slices of the volume in the scene.
    reduce(
        &mut state,
        Action::Scroll {
            viewport: 1,
            delta: 30,
        },
    );
    assert_eq!(slices(&state), [29, 45, 5, 2], "it does not bounce back");
    let held = position(&state);
    assert!(
        (held.z - 147.5).abs() < 1.0e-9,
        "the position stops at the volume's face"
    );
    assert_eq!(
        position_in_viewport(&state, 1),
        None,
        "which is not on slice 45"
    );
    assert!(position_in_viewport(&state, 0).is_some());
}

#[test]
fn going_to_a_slice_and_playing_take_the_position_too() {
    let mut state = four();
    reduce(
        &mut state,
        Action::SetSlice {
            viewport: 0,
            slice: 4,
        },
    );
    assert_eq!(slices(&state), [4, 4, 5, 2]);
    assert!((position(&state).z - 20.0).abs() < 1.0e-9);
    reduce(&mut state, Action::CineToggle { viewport: 0 });
    reduce(&mut state, Action::CineTick { viewport: 0 });
    assert_eq!(slices(&state), [5, 5, 5, 2]);
    assert!((position(&state).z - 25.0).abs() < 1.0e-9);
}

#[test]
fn a_series_of_another_study_or_with_no_place_neither_follows_nor_leads() {
    let mut state = four();
    let was = position(&state);
    for viewport in [2, 3] {
        reduce(&mut state, Action::Scroll { viewport, delta: 1 });
        assert_eq!(position(&state), was, "viewport {viewport} does not lead");
        assert_eq!(position_in_viewport(&state, viewport), None);
    }
    assert_eq!(slices(&state), [15, 15, 6, 3]);
    reduce(&mut state, Action::SetPosition(Point::new(1.0, 1.0, 101.0)));
    assert_eq!(slices(&state), [20, 20, 6, 3], "nor follow");
}

#[test]
fn the_position_is_found_in_each_linked_viewport_that_shows_its_slice() {
    let mut state = four();
    reduce(&mut state, Action::SetPosition(Point::new(1.0, 2.0, 101.0)));
    let ear = placement(&state, FINE_SCAN);
    let (expected, _) = position_on_slice(&ear, 20, position(&state)).expect("found");
    assert_eq!(position_in_viewport(&state, 1), Some(expected));
    let head = placement(&state, THICK_SCAN);
    let (expected, away) = position_on_slice(&head, 20, position(&state)).expect("found");
    assert!(
        (away - 0.2).abs() < 1.0e-9,
        "a millimetre past a slice 5 mm thick"
    );
    assert_eq!(position_in_viewport(&state, 0), Some(expected));
    assert_eq!(position_in_viewport(&state, 9), None, "no such viewport");
    reduce(&mut state, Action::SetLinked(false));
    assert_eq!(position_in_viewport(&state, 1), None, "only while linked");
}

#[test]
fn a_volume_made_ready_starts_the_position_on_the_slice_its_viewport_shows() {
    let mut state = four();
    reduce(
        &mut state,
        Action::Scroll {
            viewport: 1,
            delta: 25,
        },
    );
    assert_eq!(slices(&state), [29, 40, 5, 2]);
    // The person turns to the other series: the scene follows, and its volume is built.
    reduce(&mut state, Action::SetActive(1));
    assert_eq!(state.scene.series, Some(id(FINE_SCAN)));
    assert_eq!(state.planes.position, None, "nowhere while it is built");
    let effects = made_ready(&mut state, FINE_SCAN, 64.0);
    let ear = placement(&state, FINE_SCAN);
    assert_eq!(position(&state), ear.apply(Point::new(4.0, 4.0, 40.0)));
    assert_eq!(
        slices(&state),
        [29, 40, 5, 2],
        "the viewport looked at does not jump"
    );
    let find = Effect::FindKept {
        series: id(FINE_SCAN),
    };
    assert_eq!(
        effects,
        [find],
        "and the other is already on the nearest slice it has: only a result kept is looked for"
    );
}
