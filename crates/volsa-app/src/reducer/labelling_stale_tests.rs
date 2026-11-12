//! Labelling in the state: what a worker says of a run or a look that is no longer the one
//! waited for is old news, and changes nothing.

use std::path::PathBuf;

use volsa_segment::Stage;

use super::labelling_result_tests::{by_volsa, read_with};
use super::labelling_tests::{
    able, cards, failure, label, looked, nothing_runs, open, running, said, stage,
};
use super::*;
use crate::labelling::{LabellingAction, Ready};
use crate::labels::LabelsStatus;
use crate::state::NoticeKind;

fn stop(state: &mut AppState) {
    reduce(state, Action::Labelling(LabellingAction::StopLabelling));
}

/// A second run on, the first having been stopped.
fn second_run() -> AppState {
    let mut state = running();
    stop(&mut state);
    assert_eq!(label(&mut state).len(), 1);
    assert_eq!(state.labelling.runs, 2);
    state
}

/// What a worker reports of labels that fill the scene's volume.
fn result(state: &AppState, run: Option<u64>) -> Action {
    let volume = state.scene.corners.expect("the volume is built");
    read_with(volume, Some(by_volsa()), run)
}

#[test]
fn a_result_on_its_way_when_its_run_was_stopped_is_not_taken_for_the_file_opened_after() {
    let mut state = running();
    // Opening a file stops the run, and the file is read.
    reduce(&mut state, open());
    assert_eq!(state.labels.status, LabelsStatus::Reading);
    let before = state.clone();
    let late = result(&state, Some(1));
    assert_eq!(reduce(&mut state, late), []);
    assert_eq!(state, before, "the run's labels are not the file's");
    // The file's own labels, which carry no run, are taken as they always were.
    let file = read_with(state.scene.corners.expect("built"), None, None);
    reduce(&mut state, file);
    assert_eq!(state.labels.status, LabelsStatus::Ready);
    assert_eq!(state.labels.made, None);
}

#[test]
fn a_result_of_an_older_run_is_not_taken_for_the_newer_one() {
    let mut state = second_run();
    let before = state.clone();
    let late = result(&state, Some(1));
    assert_eq!(reduce(&mut state, late), []);
    assert_eq!(state, before);
    assert_eq!(state.labelling.running, Some(Stage::Preparing));
    // The newer run's own result is taken.
    let own = result(&state, Some(2));
    reduce(&mut state, own);
    assert!(nothing_runs(&state));
    assert_eq!(state.labels.status, LabelsStatus::Ready);
    // And a result that comes after that, of the same run, is news to nobody.
    let before = state.clone();
    let again = result(&state, Some(2));
    reduce(&mut state, again);
    assert_eq!(state, before);
}

#[test]
fn a_stage_of_an_older_run_does_not_move_the_newer_one_on() {
    let mut state = second_run();
    let old = LabellingAction::LabellingStage {
        run: 1,
        stage: Stage::Checking,
    };
    let before = state.clone();
    assert_eq!(reduce(&mut state, Action::Labelling(old)), []);
    assert_eq!(state, before);
    stage(&mut state, Stage::Checking);
    assert_eq!(state.labelling.running, Some(Stage::Checking));
}

#[test]
fn a_failure_of_an_older_run_does_not_end_the_newer_one() {
    let mut state = second_run();
    let old = LabellingAction::LabellingFailed {
        run: 1,
        why: "There was not enough memory to run the network.".to_owned(),
    };
    let before = state.clone();
    assert_eq!(reduce(&mut state, Action::Labelling(old)), []);
    assert_eq!(state, before, "the run goes on and nothing is said");
    failure(&mut state, "The model found no brain in this scan.");
    assert!(nothing_runs(&state));
    let error = (
        NoticeKind::Error,
        "The model found no brain in this scan.".to_owned(),
    );
    assert_eq!(said(&state), [error]);
}

#[test]
fn what_an_older_look_found_does_not_take_the_place_of_what_the_newest_found() {
    let mut state = able();
    let folder = |name: &str| PathBuf::from(name);
    let models = LabellingAction::SetModelsFolder(folder("first models"));
    reduce(&mut state, Action::Labelling(models));
    let runtime = LabellingAction::SetRuntimeFolder(folder("runtime"));
    reduce(&mut state, Action::Labelling(runtime));
    assert_eq!(state.labelling.looks, 2);
    let found = |look, ready| LabellingAction::LabellingLooked {
        look,
        ready,
        cards: cards(),
    };
    // The first look, made before the runtime's folder was chosen, says yes too late.
    let before = state.clone();
    reduce(&mut state, Action::Labelling(found(1, Ready::Yes)));
    assert_eq!(state, before);
    assert_eq!(state.labelling.ready, Ready::Unknown);
    let no = Ready::No("The runtime is not in the folder given.".to_owned());
    reduce(&mut state, Action::Labelling(found(2, no.clone())));
    assert_eq!(state.labelling.ready, no);
    reduce(&mut state, Action::Labelling(found(1, Ready::Yes)));
    assert_eq!(
        state.labelling.ready, no,
        "still what the newest look found"
    );
}

#[test]
fn a_look_is_asked_for_by_an_action_of_its_own_with_the_folders_as_they_are() {
    let mut state = AppState::default();
    let effects = reduce(&mut state, Action::Labelling(LabellingAction::Look));
    let look = Effect::LookForLabelling {
        look: 1,
        models: None,
        runtime: None,
    };
    assert_eq!(effects, [look]);
    looked(&mut state, Ready::Yes);
    assert_eq!(state.labelling.ready, Ready::Yes);
    // Looking again forgets what was found until the answer comes.
    let effects = reduce(&mut state, Action::Labelling(LabellingAction::Look));
    assert_eq!(effects.len(), 1);
    assert_eq!(state.labelling.ready, Ready::Unknown);
}
