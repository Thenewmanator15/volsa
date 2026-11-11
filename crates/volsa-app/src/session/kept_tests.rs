//! What is kept from one session to the next, for labelling: the result kept for a series,
//! found once its volume is ready, shown and deleted on request; and what is chosen.

use std::path::Path;

use voxa_models::RunsOn;
use voxa_segment::ModelChoice;

use super::tests::{open_until, ready_in_3d, session, session_keeping, until, zip_of};
use crate::action::Action;
use crate::choices::{Choices, RunsOnKept, load_choices, save_choices};
use crate::kept_labels::{keep, kept};
use crate::labelling::LabellingAction;
use crate::labels::LabelsStatus;
use crate::testing::{FINE_SCAN, id, three_series_files, written_labels};
use crate::views::series_key;

fn act(session: &mut super::Session, action: LabellingAction) {
    session.dispatch(Action::Labelling(action));
}

/// A result kept for the 64-slice series under this settings folder.
fn keep_for_the_fine_series(settings: &Path) {
    let (labels, table) = written_labels();
    keep(settings, &series_key(&id(FINE_SCAN)), &labels, &table).expect("it is kept");
}

#[test]
fn a_kept_result_is_found_when_the_volume_is_ready_shown_on_request_and_deleted_on_request() {
    let settings = tempfile::tempdir().expect("a temporary folder");
    keep_for_the_fine_series(settings.path());
    let (_folder, path) = zip_of(&three_series_files());
    let mut session = session_keeping(settings.path());
    open_until(&mut session, &path, |session| {
        session.state().labelling.kept.is_some()
    });
    assert!(ready_in_3d(&session, FINE_SCAN));
    assert_eq!(
        session.state().labelling.kept.as_deref(),
        Some("2099-01-02")
    );
    assert_eq!(
        session.state().labels.status,
        LabelsStatus::None,
        "not shown yet"
    );

    // Show it: its two files are read and the labels are open on the series.
    act(&mut session, LabellingAction::ShowKept);
    until(&mut session, |session| {
        session.state().labels.status != LabelsStatus::Reading
    });
    let state = session.state();
    assert_eq!(
        state.labels.status,
        LabelsStatus::Ready,
        "{:?}",
        state.notices
    );
    let made = state.labels.made.as_ref().expect("Voxa made them");
    assert_eq!(made.when, "2099-01-02");
    assert_eq!(state.labels.parts.len(), 2);

    // Delete it: the files go, and the state says nothing is kept; the open labels stay.
    act(&mut session, LabellingAction::DeleteKept);
    assert_eq!(session.state().labelling.kept, None);
    let key = series_key(&id(FINE_SCAN));
    until(&mut session, |_| kept(settings.path(), &key).is_none());
    assert_eq!(session.state().labels.status, LabelsStatus::Ready);
}

#[test]
fn a_series_with_nothing_kept_is_not_said_to_have_anything_kept() {
    let settings = tempfile::tempdir().expect("a temporary folder");
    let (_folder, path) = zip_of(&three_series_files());
    let mut session = session_keeping(settings.path());
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    session.wait(std::time::Duration::from_millis(300));
    assert_eq!(session.state().labelling.kept, None);
    assert_eq!(session.state().notices, []);
}

#[test]
fn what_was_chosen_in_the_run_before_is_chosen_again_with_its_card_waiting_to_be_found() {
    let settings = tempfile::tempdir().expect("a temporary folder");
    let before = Choices {
        choice: ModelChoice::Robust,
        cortex: false,
        runs_on: RunsOnKept::Card("Made-up card B".to_owned()),
        keep: false,
        models: Some("made-up models".into()),
        runtime: Some("made-up runtime".into()),
    };
    save_choices(settings.path(), &before).expect("they are saved");
    let session = session_keeping(settings.path());
    let labelling = &session.state().labelling;
    assert_eq!(labelling.choice, ModelChoice::Robust);
    assert_eq!((labelling.cortex, labelling.keep), (false, false));
    assert_eq!(
        labelling.models.as_deref(),
        Some(Path::new("made-up models"))
    );
    assert_eq!(
        labelling.runtime.as_deref(),
        Some(Path::new("made-up runtime"))
    );
    assert_eq!(labelling.runs_on, RunsOn::Automatic);
    assert_eq!(labelling.card_kept.as_deref(), Some("Made-up card B"));
}

#[test]
fn with_nowhere_to_keep_settings_the_choices_are_the_usual_ones() {
    let session = session();
    assert_eq!(
        session.state().labelling,
        crate::labelling::Labelling::default()
    );
}

#[test]
fn a_choice_made_is_written_and_is_there_in_the_next_session() {
    let settings = tempfile::tempdir().expect("a temporary folder");
    let mut session = session_keeping(settings.path());
    act(&mut session, LabellingAction::SetCortex(false));
    act(&mut session, LabellingAction::SetRunsOn(RunsOn::Processor));
    let written = |_: &super::Session| {
        let now = load_choices(settings.path());
        !now.cortex && now.runs_on == RunsOnKept::Processor
    };
    until(&mut session, written);
    drop(session);
    let again = session_keeping(settings.path());
    let labelling = &again.state().labelling;
    assert_eq!(
        (labelling.cortex, labelling.runs_on),
        (false, RunsOn::Processor)
    );
}
