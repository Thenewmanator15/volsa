//! Labelling in the state: a run belongs to the series it was started on, and the state ends
//! it when the scene moves to another series, since the worker ends it without a word.

use super::labelling_result_tests::{by_voxa, read_with};
use super::labelling_tests::{able, failure, nothing_runs, running, said};
use super::scene_tests::select;
use super::*;
use crate::labels::LabelsStatus;
use crate::state::{Layout, NoticeKind};
use crate::testing::{FINE_SCAN, THICK_SCAN, id};

/// What the worker says of a run that lost its volume.
const CHANGED: &str = "Labelling stopped because the scan in view changed.";

/// How many times the effects ask the worker to stop labelling.
fn stops(effects: &[Effect]) -> usize {
    let stop = |effect: &&Effect| **effect == Effect::StopLabelling;
    effects.iter().filter(stop).count()
}

#[test]
fn choosing_another_series_while_a_run_is_on_ends_it_and_asks_the_worker_to_drop_it() {
    let mut state = running();
    let effects = select(&mut state, 0, FINE_SCAN);
    assert_eq!(stops(&effects), 1);
    let build = Effect::BuildVolume {
        series: id(FINE_SCAN),
    };
    assert!(effects.contains(&build), "the scene still moves on");
    assert_eq!(state.scene.series, Some(id(FINE_SCAN)));
    assert!(nothing_runs(&state));
    assert_eq!(state.labelling.runs, 1);
    assert_eq!(said(&state), [], "the person moved on themselves");
    // Back on the first series, nothing is running to stop.
    let back = select(&mut state, 0, THICK_SCAN);
    assert_eq!(stops(&back), 0);
    assert!(nothing_runs(&state));
}

#[test]
fn choosing_the_same_series_again_leaves_the_run_going() {
    let mut state = running();
    let before = state.labelling.clone();
    let effects = select(&mut state, 0, THICK_SCAN);
    assert_eq!(stops(&effects), 0);
    assert_eq!(state.labelling, before);
    assert_eq!(said(&state), []);
}

#[test]
fn choosing_a_series_when_no_run_is_on_asks_for_no_stop() {
    let mut state = able();
    let before = state.labelling.clone();
    let effects = select(&mut state, 0, FINE_SCAN);
    assert_eq!(stops(&effects), 0);
    assert_eq!(state.labelling, before);
    assert_eq!(said(&state), []);
}

#[test]
fn a_scene_that_shows_nothing_leaves_the_run_going_for_its_series() {
    let mut state = running();
    reduce(&mut state, Action::SetLayout(Layout::TwoAcross));
    let effects = reduce(&mut state, Action::SetActive(1));
    assert_eq!(state.scene.series, None);
    assert_eq!(stops(&effects), 0);
    assert!(state.labelling.running.is_some() && state.labelling.on.is_some());
    // Back on the series it labels, it goes on as it was.
    let effects = reduce(&mut state, Action::SetActive(0));
    assert_eq!(stops(&effects), 0);
    assert_eq!(state.labelling.on, Some(id(THICK_SCAN)));
}

#[test]
fn labels_on_their_way_from_a_run_the_scene_left_are_not_taken() {
    let mut state = running();
    let volume = state.scene.corners.expect("the volume is built");
    // Another series of the same study: the run is over all the same.
    select(&mut state, 0, FINE_SCAN);
    let before = state.labels.clone();
    let late = read_with(volume, Some(by_voxa()), Some(1));
    assert_eq!(reduce(&mut state, late), []);
    assert_eq!(state.labels, before);
    assert_eq!(state.labels.status, LabelsStatus::None);
}

#[test]
fn a_runs_labels_that_lie_nowhere_near_the_volume_are_refused_and_never_held() {
    let mut state = running();
    let volume = state.scene.corners.expect("the volume is built");
    let far = volume.map(|corner| voxa_core::Point::new(corner.x + 900.0, corner.y, corner.z));
    let effects = reduce(&mut state, read_with(far, Some(by_voxa()), Some(1)));
    assert_eq!(effects, [Effect::DropLabels]);
    assert!(nothing_runs(&state));
}

#[test]
fn the_workers_are_asked_to_hold_a_runs_labels_only_when_the_state_takes_them() {
    let volume = running().scene.corners.expect("the volume is built");
    let late = || read_with(volume, Some(by_voxa()), Some(1));
    let mut taken = running();
    assert_eq!(reduce(&mut taken, late()), [Effect::TakeLabels { run: 1 }]);
    // Stop is pressed, or the scene moves to another series, between the worker's report and
    // the state's taking it: the worker is asked to stop, which lets the labels go, and never
    // to hold them.
    let mut stopped = running();
    let stop = Action::Labelling(crate::labelling::LabellingAction::StopLabelling);
    let stop = reduce(&mut stopped, stop);
    let mut moved = running();
    let moved_on = select(&mut moved, 0, FINE_SCAN);
    for (state, effects) in [(&mut stopped, stop), (&mut moved, moved_on)] {
        assert_eq!(stops(&effects), 1);
        assert_eq!(reduce(state, late()), [], "nothing to hold");
    }
}

#[test]
fn the_workers_word_that_the_scan_changed_is_old_news_once_the_state_ended_the_run() {
    let mut state = running();
    select(&mut state, 0, FINE_SCAN);
    let before = state.clone();
    assert_eq!(failure(&mut state, CHANGED), []);
    assert_eq!(state, before, "nothing is said twice");
}

#[test]
fn a_run_whose_volume_was_made_again_under_the_same_series_ends_on_the_workers_word() {
    let mut state = running();
    // The index is done: the scene is built afresh for the series it was on.
    let effects = reduce(&mut state, Action::IndexDone);
    assert_eq!(stops(&effects), 0);
    assert!(state.labelling.running.is_some(), "the series is the same");
    assert_eq!(failure(&mut state, CHANGED), []);
    assert!(nothing_runs(&state));
    assert_eq!(said(&state), [(NoticeKind::Error, CHANGED.to_owned())]);
}
