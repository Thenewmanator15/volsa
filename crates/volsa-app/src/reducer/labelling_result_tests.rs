//! Labelling in the state: what a finished run brings back, and what stops a run.

use std::path::PathBuf;

use volsa_core::{Patient, Point};
use volsa_dicom::testkit::{SliceOptions, classic_slice};
use volsa_labels::{Made, Part, Side, Trust};
use volsa_segment::{ModelChoice, Stage};

use super::labelling_tests::{
    STOPPED, able, cards, label, looked, nothing_runs, open, reading_labels, running, said,
};
use super::scene_tests::{corners, select};
use super::*;
use crate::labelling::{LabellingAction, Ready};
use crate::labels::{Labels, LabelsStatus, NamesFrom, on_this_series, study_of};
use crate::state::{Layout, NoticeKind};
use crate::testing::{FINE_SCAN, THICK_SCAN, id, indexed, series_files, slice_options};

fn parts() -> Vec<Part> {
    let part = |side| Part {
        name: "hippocampus".to_owned(),
        group: "Deep brain".to_owned(),
        side,
        colour: [10, 20, 30],
        trust: Trust::Model,
        voxels: 1000,
        ml: 1.0,
    };
    vec![part(Side::Left), part(Side::Right)]
}

/// What a worker reports of labels that fill `corners`, made by Volsa or not, by a run or
/// read from a file.
pub(super) fn read_with(
    corners: [Point<Patient>; 8],
    made: Option<Made>,
    run: Option<u64>,
) -> Action {
    Action::LabelsRead {
        title: "A made-up head".to_owned(),
        parts: parts(),
        verdict: None,
        corners,
        names_from: NamesFrom::Table,
        made,
        run,
    }
}

/// Who made labels: Volsa.
pub(super) fn by_volsa() -> Made {
    Made {
        by: "Volsa 9.8.7".to_owned(),
        ..Made::default()
    }
}

/// What a worker reports of the first run, finished, whose labels fill `corners`.
fn came_back(corners: [Point<Patient>; 8]) -> Action {
    read_with(corners, Some(by_volsa()), Some(1))
}

/// A run on, with labels of the same study already open.
fn running_with_labels() -> AppState {
    let mut state = able();
    reduce(&mut state, open());
    let volume = state.scene.corners.expect("the volume is built");
    reduce(&mut state, read_with(volume, Some(by_volsa()), None));
    assert_eq!(state.labels.status, LabelsStatus::Ready);
    assert_eq!(label(&mut state).len(), 1);
    state
}

/// The 30-slice series in one study and the 64-slice series in another.
fn two_studies() -> volsa_dicom::Index {
    let another = (0..64).map(|place| {
        let options = SliceOptions {
            study_uid: "1.2.3.99".to_owned(),
            ..slice_options(FINE_SCAN, 2, place, 8)
        };
        (format!("S2/I{place:04}.dcm"), classic_slice(&options))
    });
    indexed([series_files(THICK_SCAN, 1, 30, 4), another.collect()].concat())
}

/// The first of those two with its volume built, on a computer that can label.
fn able_with_two_studies() -> AppState {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(two_studies()));
    select(&mut state, 0, THICK_SCAN);
    let (series, corners) = (id(THICK_SCAN), corners());
    reduce(&mut state, Action::VolumeReady { series, corners });
    looked(&mut state, Ready::Yes);
    state
}

#[test]
fn labels_that_come_back_are_taken_in_as_those_of_the_study_that_was_labelled() {
    let mut state = running();
    let box_of_the_volume = state.scene.corners.expect("the volume is built");
    // The workers are asked to hold them now, and only now: what is drawn is what is listed.
    assert_eq!(
        reduce(&mut state, came_back(box_of_the_volume)),
        [Effect::TakeLabels { run: 1 }]
    );
    assert!(nothing_runs(&state));
    assert_eq!(state.labels.status, LabelsStatus::Ready);
    assert_eq!(state.labels.parts, parts());
    assert_eq!(state.labels.title, "A made-up head");
    let by = state.labels.made.as_ref().map(|made| made.by.as_str());
    assert_eq!(by, Some("Volsa 9.8.7"));
    assert_eq!(state.labels.study, study_of(&state, &id(THICK_SCAN)));
    assert!(on_this_series(&state));
}

#[test]
fn labels_that_come_back_are_kept_when_the_scene_shows_nothing() {
    // The scene shows nothing: the labels wait for their study. (A scene that moves to another
    // series of the same study ends the run instead: see the series tests.)
    let mut state = running();
    reduce(&mut state, Action::SetLayout(Layout::TwoAcross));
    reduce(&mut state, Action::SetActive(1));
    assert_eq!(state.scene.series, None);
    reduce(&mut state, came_back(corners()));
    assert_eq!(state.labels.status, LabelsStatus::Ready);
    assert!(!on_this_series(&state));
    reduce(&mut state, Action::SetActive(0));
    assert!(on_this_series(&state), "back on the study they are of");
}

#[test]
fn labels_that_come_back_after_the_scene_moved_to_another_study_are_dropped() {
    let mut state = able_with_two_studies();
    assert_eq!(label(&mut state).len(), 1);
    select(&mut state, 0, FINE_SCAN);
    let other = study_of(&state, &id(FINE_SCAN));
    assert_ne!(other, study_of(&state, &id(THICK_SCAN)), "two studies");
    let before = state.labels.clone();
    assert_eq!(reduce(&mut state, came_back(corners())), []);
    assert_eq!(state.labels, before);
    assert!(nothing_runs(&state));
}

#[test]
fn labels_that_nobody_is_waiting_for_are_ignored() {
    let mut idle = able();
    let mut stopped = running();
    reduce(
        &mut stopped,
        Action::Labelling(LabellingAction::StopLabelling),
    );
    for state in [&mut idle, &mut stopped] {
        let before = state.clone();
        reduce(state, came_back(corners()));
        assert_eq!(*state, before);
    }
}

#[test]
fn another_export_stops_a_run_and_drops_the_labels_and_keeps_the_choices() {
    let mut state = running_with_labels();
    let robust = Action::Labelling(LabellingAction::SetModelChoice(ModelChoice::Robust));
    reduce(&mut state, robust);
    state.labelling.kept = Some("2099-01-02".to_owned());
    let path = PathBuf::from("another");
    let effects = reduce(&mut state, Action::OpenPath(path.clone()));
    let asked = [Effect::StopLabelling, Effect::DropLabels];
    assert_eq!(
        effects,
        [asked.to_vec(), vec![Effect::StartIndex(path)]].concat()
    );
    assert_eq!(state.labels.status, LabelsStatus::None);
    let choices = (state.labelling.choice, state.labelling.ready.clone());
    assert_eq!(choices, (ModelChoice::Robust, Ready::Yes), "these stay");
    assert_eq!(state.labelling.cards, cards());
    assert!(nothing_runs(&state) && state.labelling.kept.is_none());
    // With a run and no labels, only the run is stopped.
    let mut state = running();
    let path = PathBuf::from("another");
    let effects = reduce(&mut state, Action::OpenPath(path.clone()));
    assert_eq!(effects, [Effect::StopLabelling, Effect::StartIndex(path)]);
}

#[test]
fn opening_labels_stops_a_run_and_the_new_file_is_read_in_place_of_the_old_labels() {
    let mut state = running_with_labels();
    let effects = reduce(&mut state, open());
    let read = Effect::ReadLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    };
    assert_eq!(effects, [Effect::StopLabelling, read]);
    assert!(nothing_runs(&state));
    assert_eq!(said(&state), [(NoticeKind::Info, STOPPED.to_owned())]);
    assert_eq!(state.labels.status, LabelsStatus::Reading);
    assert_eq!(state.labels.study, study_of(&state, &id(THICK_SCAN)));
    assert!(state.labels.parts.is_empty(), "the old labels are gone");
}

#[test]
fn closing_labels_stops_a_run_and_empties_them() {
    let mut state = running_with_labels();
    let effects = reduce(&mut state, Action::CloseLabels);
    assert_eq!(effects, [Effect::StopLabelling, Effect::DropLabels]);
    assert!(nothing_runs(&state));
    assert_eq!(said(&state), [(NoticeKind::Info, STOPPED.to_owned())]);
    assert_eq!(state.labels, Labels::default());
}

#[test]
fn a_label_file_read_while_no_run_is_on_is_a_file_and_leaves_the_run_state_alone() {
    // A run asked for while the file is being read is refused, so the file's labels are
    // never taken for a run's; with or without the mark of who made them.
    for made in [Some(by_volsa()), None] {
        let mut state = reading_labels();
        assert_eq!(label(&mut state), [], "refused while the file is read");
        let volume = state.scene.corners.expect("the volume is built");
        let effects = reduce(&mut state, read_with(volume, made.clone(), None));
        assert_eq!(effects, []);
        assert!(nothing_runs(&state), "{made:?}");
        assert_eq!(state.labels.status, LabelsStatus::Ready);
        assert_eq!(state.labels.made, made);
        assert_eq!(state.labels.study, study_of(&state, &id(THICK_SCAN)));
        assert_eq!(said(&state), []);
    }
}

#[test]
fn opening_labels_that_cannot_be_opened_leaves_the_run_going() {
    let mut state = running();
    reduce(&mut state, Action::SetLayout(Layout::TwoAcross));
    reduce(&mut state, Action::SetActive(1));
    assert_eq!(reduce(&mut state, open()), [], "no volume to put them on");
    assert_eq!(state.labelling.running, Some(Stage::Preparing));
    // With no run on, none is stopped.
    let mut idle = able();
    let read = Effect::ReadLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    };
    assert_eq!(reduce(&mut idle, open()), [read]);
    assert_eq!(said(&idle), []);
}
