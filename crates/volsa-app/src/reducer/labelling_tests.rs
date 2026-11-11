//! Labelling in the state: what is chosen, what is running, and how a run starts, goes on,
//! fails and is stopped.

use std::path::PathBuf;

use voxa_models::{Card, CardId, CardKind, RunsOn, Wanted};
use voxa_segment::{ModelChoice, Stage};

use super::tests::showing;
use super::*;
use crate::labelling::{Labelling, LabellingAction, Ready};
use crate::state::NoticeKind;
use crate::testing::{THICK_SCAN, id, thick_scan_ready};

/// Bytes of the graphics card Voxa itself holds, as the window would report them.
pub(super) const HELD: u64 = 1 << 30;
pub(super) const STOPPED: &str = "Labelling was stopped.";

pub(super) fn card(number: u64, name: &str, kind: CardKind) -> Card {
    Card {
        id: CardId(number),
        index: u32::try_from(number).expect("a small number"),
        name: name.to_owned(),
        kind,
        own_bytes: 8 << 30,
    }
}

/// An integrated card, then two discrete ones.
pub(super) fn cards() -> Vec<Card> {
    vec![
        card(1, "Made-up shared graphics", CardKind::Integrated),
        card(2, "Made-up card A", CardKind::Discrete),
        card(3, "Made-up card B", CardKind::Discrete),
    ]
}

/// What the newest look found: this computer's readiness, and these cards.
pub(super) fn looked_on(state: &mut AppState, ready: Ready, cards: Vec<Card>) {
    let look = state.labelling.looks;
    let found = LabellingAction::LabellingLooked { look, ready, cards };
    reduce(state, Action::Labelling(found));
}

pub(super) fn looked(state: &mut AppState, ready: Ready) {
    looked_on(state, ready, cards());
}

/// What the run on says of itself.
pub(super) fn stage(state: &mut AppState, reached: Stage) {
    let run = state.labelling.runs;
    let said = LabellingAction::LabellingStage {
        run,
        stage: reached,
    };
    reduce(state, Action::Labelling(said));
}

/// What the run on says when it fails.
pub(super) fn failure(state: &mut AppState, why: &str) -> Vec<Effect> {
    let (run, why) = (state.labelling.runs, why.to_owned());
    let failed = LabellingAction::LabellingFailed { run, why };
    reduce(state, Action::Labelling(failed))
}

/// The 30-slice series with its volume built, on a computer that can label.
pub(super) fn able() -> AppState {
    let mut state = thick_scan_ready();
    looked(&mut state, Ready::Yes);
    state
}

pub(super) fn label(state: &mut AppState) -> Vec<Effect> {
    reduce(
        state,
        Action::Labelling(LabellingAction::LabelScan { held: HELD }),
    )
}

/// A state in which a label file is being read.
pub(super) fn reading_labels() -> AppState {
    let mut state = able();
    reduce(&mut state, open());
    state
}

/// A state with a run on.
pub(super) fn running() -> AppState {
    let mut state = able();
    assert_eq!(label(&mut state).len(), 1);
    state
}

/// The scene's series as scanned with its slices this far apart.
pub(super) fn slices_apart(state: &mut AppState, mm: f64) {
    let shown = state.scene.series.clone().expect("a series in the scene");
    let index = state.index.as_mut().expect("indexed");
    for listed in index.series.iter_mut().filter(|one| one.info.id == shown) {
        listed.info.spacing_mm = [0.75, 0.75, mm];
    }
}

/// Whether no run is on: none running, and no series being labelled.
pub(super) fn nothing_runs(state: &AppState) -> bool {
    state.labelling.running.is_none() && state.labelling.on.is_none()
}

pub(super) fn open() -> Action {
    Action::OpenLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    }
}

pub(super) fn said(state: &AppState) -> Vec<(NoticeKind, String)> {
    let notices = state.notices.iter();
    notices
        .map(|notice| (notice.kind, notice.text.clone()))
        .collect()
}

#[test]
fn a_fresh_state_has_looked_at_nothing_and_has_the_usual_choices() {
    let expected = Labelling {
        ready: Ready::Unknown,
        cards: Vec::new(),
        drawing_on: None,
        choice: ModelChoice::Automatic,
        cortex: true,
        runs_on: RunsOn::Automatic,
        keep: true,
        models: None,
        runtime: None,
        running: None,
        on: None,
        kept: None,
        kept_files: None,
        card_kept: None,
        runs: 0,
        looks: 0,
    };
    assert_eq!(AppState::default().labelling, expected);
}

#[test]
fn labelling_starts_when_ready_with_a_volume_and_asks_for_the_run() {
    let mut state = able();
    reduce(
        &mut state,
        Action::Labelling(LabellingAction::DrawingOn("Made-up card A".to_owned())),
    );
    let effects = label(&mut state);
    let asked = Effect::LabelScan {
        run: 1,
        series: id(THICK_SCAN),
        wanted: Wanted {
            robust: true,
            cortex: true,
        },
        runs_on: RunsOn::Automatic,
        held: HELD,
        keep: true,
        drawing_on: Some("Made-up card A".to_owned()),
    };
    assert_eq!(effects, [asked]);
    assert_eq!(state.labelling.running, Some(Stage::Preparing));
    assert_eq!(state.labelling.on, Some(id(THICK_SCAN)));
}

#[test]
fn the_run_carries_every_choice() {
    let mut state = able();
    let chosen = [
        Action::Labelling(LabellingAction::SetModelChoice(ModelChoice::Standard)),
        Action::Labelling(LabellingAction::SetCortex(false)),
        Action::Labelling(LabellingAction::SetRunsOn(RunsOn::Card(CardId(3)))),
        Action::Labelling(LabellingAction::SetKeep(false)),
    ];
    for action in chosen {
        // Each choice is asked to be kept (see the tests of what is kept) and nothing more.
        let effects = reduce(&mut state, action);
        assert!(matches!(effects.as_slice(), [Effect::StoreChoices(_)]));
    }
    let asked = Effect::LabelScan {
        run: 1,
        series: id(THICK_SCAN),
        wanted: Wanted {
            robust: false,
            cortex: false,
        },
        runs_on: RunsOn::Card(CardId(3)),
        held: HELD,
        keep: false,
        drawing_on: None,
    };
    assert_eq!(label(&mut state), [asked]);
}

#[test]
fn the_robust_model_is_asked_for_by_hand_or_by_slices_far_apart() {
    use ModelChoice::{Automatic, Robust, Standard};
    let cases = [
        (Automatic, 1.0, false),
        (Automatic, 3.0, false),
        (Automatic, 6.0, true),
        (Standard, 6.0, false),
        (Robust, 1.0, true),
    ];
    for (choice, mm, robust) in cases {
        let mut state = able();
        slices_apart(&mut state, mm);
        reduce(
            &mut state,
            Action::Labelling(LabellingAction::SetModelChoice(choice)),
        );
        let effects = label(&mut state);
        let Some(Effect::LabelScan { wanted, .. }) = effects.first() else {
            panic!("a run was asked for: {effects:?}");
        };
        assert_eq!(wanted.robust, robust, "{choice:?}, {mm} mm");
    }
}

#[test]
fn nothing_starts_unless_ready_idle_and_with_a_volume() {
    let mut not_possible = thick_scan_ready();
    looked(&mut not_possible, Ready::No("No models.".to_owned()));
    let mut building = showing(THICK_SCAN);
    looked(&mut building, Ready::Yes);
    let mut nothing_open = AppState::default();
    looked(&mut nothing_open, Ready::Yes);
    let cases = [
        ("never looked", thick_scan_ready()),
        ("not possible", not_possible),
        ("already running", running()),
        ("a label file still being read", reading_labels()),
        ("volume still being built", building),
        ("nothing open", nothing_open),
    ];
    for (why, mut state) in cases {
        let before = state.clone();
        assert_eq!(label(&mut state), [], "{why}");
        assert_eq!(state, before, "{why}");
    }
}

#[test]
fn a_run_says_how_far_it_has_got_and_ignores_news_of_one_that_is_over() {
    let mut state = running();
    let finding = Stage::Finding { step: 2, of: 3 };
    stage(&mut state, finding);
    assert_eq!(state.labelling.running, Some(finding));
    reduce(
        &mut state,
        Action::Labelling(LabellingAction::StopLabelling),
    );
    let before = state.clone();
    stage(&mut state, Stage::Checking);
    assert_eq!(state, before, "a late word from a run that was stopped");
}

#[test]
fn a_stopped_run_is_cleared_with_a_word_and_asked_to_stop_once() {
    let mut state = running();
    let effects = reduce(
        &mut state,
        Action::Labelling(LabellingAction::StopLabelling),
    );
    assert_eq!(effects, [Effect::StopLabelling]);
    assert!(nothing_runs(&state));
    assert_eq!(said(&state), [(NoticeKind::Info, STOPPED.to_owned())]);
    // Nothing is running now: pressing stop again says and asks for nothing.
    let before = state.clone();
    assert_eq!(
        reduce(
            &mut state,
            Action::Labelling(LabellingAction::StopLabelling)
        ),
        []
    );
    assert_eq!(state, before);
}

#[test]
fn a_failed_run_is_cleared_and_the_sentence_is_posted_as_an_error() {
    let mut state = running();
    let why = "There is not enough memory to label this scan.";
    let effects = failure(&mut state, why);
    assert_eq!(effects, []);
    assert!(nothing_runs(&state));
    assert_eq!(said(&state), [(NoticeKind::Error, why.to_owned())]);
    // A new run takes the old word away; a failure nobody is waiting for says nothing.
    label(&mut state);
    assert_eq!(said(&state), []);
    reduce(
        &mut state,
        Action::Labelling(LabellingAction::StopLabelling),
    );
    let before = state.clone();
    failure(&mut state, "Late.");
    assert_eq!(state, before);
}

#[test]
fn what_was_looked_at_and_each_choice_is_kept() {
    let mut state = AppState::default();
    let ready = Ready::No("No models.".to_owned());
    let looked_at = Action::Labelling(LabellingAction::LabellingLooked {
        look: 0,
        ready: ready.clone(),
        cards: cards(),
    });
    assert_eq!(reduce(&mut state, looked_at), []);
    assert_eq!(
        (&state.labelling.ready, &state.labelling.cards),
        (&ready, &cards())
    );
    let chosen = [
        Action::Labelling(LabellingAction::SetModelChoice(ModelChoice::Standard)),
        Action::Labelling(LabellingAction::SetCortex(false)),
        Action::Labelling(LabellingAction::SetRunsOn(RunsOn::Processor)),
        Action::Labelling(LabellingAction::SetKeep(false)),
        Action::Labelling(LabellingAction::DrawingOn("Made-up card B".to_owned())),
    ];
    for action in chosen {
        reduce(&mut state, action);
    }
    let labelling = &state.labelling;
    assert_eq!(labelling.choice, ModelChoice::Standard);
    assert_eq!((labelling.cortex, labelling.keep), (false, false));
    assert_eq!(labelling.runs_on, RunsOn::Processor);
    assert_eq!(labelling.drawing_on.as_deref(), Some("Made-up card B"));
    reduce(
        &mut state,
        Action::Labelling(LabellingAction::DrawingOn(String::new())),
    );
    assert_eq!(
        state.labelling.drawing_on, None,
        "a card with no name is none"
    );
}

#[test]
fn changing_a_folder_keeps_it_and_asks_to_look_again() {
    let mut state = able();
    let (models, runtime) = (PathBuf::from("models"), PathBuf::from("runtime"));
    let effects = reduce(
        &mut state,
        Action::Labelling(LabellingAction::SetModelsFolder(models.clone())),
    );
    let look = Effect::LookForLabelling {
        look: 1,
        models: Some(models.clone()),
        runtime: None,
    };
    assert_eq!(effects.last(), Some(&look));
    assert_eq!(effects.len(), 2, "and the folder is asked to be kept");
    assert_eq!(state.labelling.models, Some(models.clone()));
    assert_eq!(
        state.labelling.ready,
        Ready::Unknown,
        "until it is looked at"
    );
    let effects = reduce(
        &mut state,
        Action::Labelling(LabellingAction::SetRuntimeFolder(runtime.clone())),
    );
    let look = Effect::LookForLabelling {
        look: 2,
        models: Some(models),
        runtime: Some(runtime.clone()),
    };
    assert_eq!(effects.last(), Some(&look));
    assert_eq!(effects.len(), 2, "and the folder is asked to be kept");
    assert_eq!(state.labelling.runtime, Some(runtime));
}
