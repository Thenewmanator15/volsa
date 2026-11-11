//! Labelling in the state: the result kept for the scene's series, asked for when its volume
//! is ready, and then found, shown, deleted and saved.

use std::path::PathBuf;

use super::labelling_result_tests::{by_voxa, read_with};
use super::labelling_tests::{able, label, open, running};
use super::scene_tests::{corners, select};
use super::tests::showing;
use super::*;
use crate::labelling::LabellingAction;
use crate::labels::{LabelsStatus, study_of};
use crate::testing::{FINE_SCAN, THICK_SCAN, id};

fn act(state: &mut AppState, action: LabellingAction) -> Vec<Effect> {
    reduce(state, Action::Labelling(action))
}

fn labels_file() -> PathBuf {
    PathBuf::from("kept").join("labels.nii.gz")
}

fn table_file() -> PathBuf {
    PathBuf::from("kept").join("labels.json")
}

/// What a worker says on finding a result kept for a series, made on this day.
fn found_on(series: &str, when: &str) -> LabellingAction {
    LabellingAction::KeptFound {
        series: id(series),
        when: when.to_owned(),
        labels: labels_file(),
        table: table_file(),
    }
}

fn found(series: &str) -> LabellingAction {
    found_on(series, "2099-01-02")
}

/// The 30-slice series with its volume built, and a result found kept for it.
fn with_a_kept_result() -> AppState {
    let mut state = able();
    assert_eq!(act(&mut state, found(THICK_SCAN)), []);
    state
}

/// Nothing is kept, as far as the state knows.
fn nothing_kept(state: &AppState) -> bool {
    state.labelling.kept.is_none() && state.labelling.kept_files.is_none()
}

/// Labels of the scene's study, made by Voxa, open on it.
fn open_labels(state: &mut AppState) {
    reduce(state, open());
    let volume = state.scene.corners.expect("the volume is built");
    reduce(state, read_with(volume, Some(by_voxa()), None));
    assert_eq!(state.labels.status, LabelsStatus::Ready);
}

#[test]
fn a_volume_that_is_ready_asks_for_the_result_kept_for_its_series() {
    let mut state = showing(THICK_SCAN);
    let ready = Action::VolumeReady {
        series: id(THICK_SCAN),
        corners: corners(),
    };
    let find = Effect::FindKept {
        series: id(THICK_SCAN),
    };
    assert_eq!(reduce(&mut state, ready.clone()), [find]);
    let again = reduce(&mut state, ready);
    assert_eq!(again, [], "a volume reported twice is asked about once");
}

#[test]
fn a_volume_the_scene_does_not_show_asks_for_nothing() {
    let mut state = showing(THICK_SCAN);
    let ready = Action::VolumeReady {
        series: id(FINE_SCAN),
        corners: corners(),
    };
    assert_eq!(reduce(&mut state, ready), []);
}

#[test]
fn what_is_found_kept_is_remembered_for_the_series_in_the_scene() {
    let state = with_a_kept_result();
    assert_eq!(state.labelling.kept.as_deref(), Some("2099-01-02"));
    let files = (labels_file(), table_file());
    assert_eq!(state.labelling.kept_files, Some(files));
}

#[test]
fn a_result_found_for_a_series_the_scene_no_longer_shows_is_old_news() {
    let mut state = able();
    let before = state.clone();
    assert_eq!(act(&mut state, found(FINE_SCAN)), []);
    assert_eq!(state, before);
}

#[test]
fn another_series_in_the_scene_has_nothing_kept_until_its_own_is_found() {
    let mut state = with_a_kept_result();
    select(&mut state, 0, FINE_SCAN);
    assert!(nothing_kept(&state));
    // What was asked about the first series, and answered late, is not the second's.
    act(&mut state, found(THICK_SCAN));
    assert!(nothing_kept(&state));
}

#[test]
fn another_export_starts_with_nothing_kept() {
    let mut state = with_a_kept_result();
    reduce(
        &mut state,
        Action::OpenPath(PathBuf::from("another export")),
    );
    assert!(nothing_kept(&state));
}

#[test]
fn showing_the_kept_result_reads_its_two_files_as_labels_on_the_study() {
    let mut state = with_a_kept_result();
    let effects = act(&mut state, LabellingAction::ShowKept);
    let read = Effect::ReadLabels {
        labels: labels_file(),
        table: Some(table_file()),
    };
    assert_eq!(effects, [read]);
    assert_eq!(state.labels.status, LabelsStatus::Reading);
    assert_eq!(state.labels.study, study_of(&state, &id(THICK_SCAN)));
    // They come back as labels read from a file, and are taken in as such.
    let volume = state.scene.corners.expect("the volume is built");
    reduce(&mut state, read_with(volume, Some(by_voxa()), None));
    assert_eq!(state.labels.status, LabelsStatus::Ready);
}

#[test]
fn showing_with_nothing_kept_does_nothing() {
    let mut state = able();
    let before = state.clone();
    assert_eq!(act(&mut state, LabellingAction::ShowKept), []);
    assert_eq!(state, before);
}

#[test]
fn showing_the_kept_result_while_a_run_is_on_stops_the_run_as_opening_a_file_does() {
    let mut state = running();
    act(&mut state, found(THICK_SCAN));
    let effects = act(&mut state, LabellingAction::ShowKept);
    let read = Effect::ReadLabels {
        labels: labels_file(),
        table: Some(table_file()),
    };
    assert_eq!(effects, [Effect::StopLabelling, read]);
    assert_eq!(state.labelling.running, None);
}

#[test]
fn deleting_the_kept_result_asks_for_it_to_be_forgotten_and_clears_it() {
    let mut state = with_a_kept_result();
    let effects = act(&mut state, LabellingAction::DeleteKept);
    let forget = Effect::ForgetKept {
        series: id(THICK_SCAN),
    };
    assert_eq!(effects, [forget]);
    assert!(nothing_kept(&state));
    let again = act(&mut state, LabellingAction::DeleteKept);
    assert_eq!(again, [], "what is not kept is not deleted");
}

#[test]
fn deleting_what_is_kept_leaves_the_labels_that_are_open_alone() {
    let mut state = with_a_kept_result();
    open_labels(&mut state);
    let labels = state.labels.clone();
    act(&mut state, LabellingAction::DeleteKept);
    assert_eq!(state.labels, labels);
}

#[test]
fn a_run_may_start_with_a_result_kept_and_the_date_of_the_new_one_replaces_the_old() {
    let mut state = with_a_kept_result();
    let effects = label(&mut state);
    assert!(matches!(
        effects.as_slice(),
        [Effect::LabelScan { keep: true, .. }]
    ));
    assert_eq!(state.labelling.kept.as_deref(), Some("2099-01-02"));
    // The new result is kept in place of the old, and the worker says so.
    act(&mut state, found_on(THICK_SCAN, "2099-03-04"));
    assert_eq!(state.labelling.kept.as_deref(), Some("2099-03-04"));
}

#[test]
fn saving_the_labels_that_are_open_asks_for_them_to_be_written_where_the_person_chose() {
    let mut state = able();
    open_labels(&mut state);
    let to = PathBuf::from("chosen").join("my labels.nii.gz");
    let effects = act(&mut state, LabellingAction::SaveLabelsAs(to.clone()));
    assert_eq!(effects, [Effect::SaveLabels { to }]);
}

#[test]
fn with_no_labels_open_or_still_being_read_there_is_nothing_to_save() {
    let mut state = able();
    let to = PathBuf::from("my labels.nii.gz");
    assert_eq!(
        act(&mut state, LabellingAction::SaveLabelsAs(to.clone())),
        []
    );
    reduce(&mut state, open());
    assert_eq!(state.labels.status, LabelsStatus::Reading);
    assert_eq!(act(&mut state, LabellingAction::SaveLabelsAs(to)), []);
}
