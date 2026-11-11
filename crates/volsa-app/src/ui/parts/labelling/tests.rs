//! The strip at the top of the Parts tab: what it shows in each state, and what each of its
//! buttons and menu entries asks for, found by clicking all over the tab as its list is swept.

use std::path::PathBuf;

use volsa_labels::{Made, Verdict};
use volsa_models::{Card, CardId, CardKind};
use volsa_segment::Stage;

use super::super::tests::{Shown, labelled_by, named, pass, sweep_with, written};
use super::super::{Asked, MADE_ELSEWHERE};
use super::*;
use crate::labelling::{LabellingAction, Ready, can_label, what_will_run};
use crate::labels::NamesFrom;
use crate::reducer::reduce;
use crate::testing::thick_scan_ready;

/// Bytes of the graphics card the window holds, as the strip is told.
pub(super) const HELD: u64 = 1 << 30;

pub(super) fn card(number: u64, name: &str, kind: CardKind) -> Card {
    Card {
        id: CardId(number),
        index: u32::try_from(number).expect("a small number"),
        name: name.to_owned(),
        kind,
        own_bytes: 8 << 30,
    }
}

/// An integrated card, a discrete one and a software one.
pub(super) fn cards() -> Vec<Card> {
    vec![
        card(1, "Made-up shared graphics", CardKind::Integrated),
        card(2, "Made-up card A", CardKind::Discrete),
        card(3, "Made-up software renderer", CardKind::Software),
    ]
}

/// The 30-slice series with its volume built, on a computer that can label, with no labels.
pub(super) fn able() -> AppState {
    let mut state = thick_scan_ready();
    state.labelling.ready = Ready::Yes;
    state.labelling.cards = cards();
    state
}

/// Who made labels: Volsa, with a model, on a card, on a day, and two notes.
pub(super) fn by_volsa() -> Made {
    Made {
        by: "Volsa 9.8.7".to_owned(),
        model: "Made-up model, robust".to_owned(),
        model_sum: String::new(),
        device: "Made-up card A".to_owned(),
        when: "2099-01-02".to_owned(),
        notes: vec![
            "A first made-up note.".to_owned(),
            "A second made-up note.".to_owned(),
        ],
        added: None,
    }
}

/// Labels of the scene's study made by Volsa, open, on a computer that can label.
pub(super) fn made_by_volsa(verdict: Option<Verdict>) -> AppState {
    let mut state = labelled_by(verdict, NamesFrom::Table, Some(by_volsa()));
    state.labelling.ready = Ready::Yes;
    state.labelling.cards = cards();
    state
}

/// Labels from a file open, and a result kept for the series.
pub(super) fn kept_with_labels_open() -> AppState {
    let mut state = named();
    state.labelling.ready = Ready::Yes;
    state.labelling.cards = cards();
    state.labelling.kept = Some("2099-01-02".to_owned());
    let (labels, table) = (PathBuf::from("labels.nii.gz"), PathBuf::from("labels.json"));
    state.labelling.kept_files = Some((labels, table));
    state
}

pub(super) fn act(action: LabellingAction) -> Action {
    Action::Labelling(action)
}

pub(super) fn label_scan() -> Action {
    act(LabellingAction::LabelScan { held: HELD })
}

pub(super) fn asks_to_label(actions: &[Action]) -> bool {
    let is_it =
        |action: &Action| matches!(action, Action::Labelling(LabellingAction::LabelScan { .. }));
    actions.iter().any(is_it)
}

/// Click all over the tab, told that the window holds `HELD` bytes of its card.
pub(super) fn swept(state: &AppState) -> (Vec<Action>, Vec<Asked>) {
    sweep_with(state, Shown::with(HELD, false))
}

/// As [`swept`], with the strip's menu held open.
pub(super) fn swept_menu(state: &AppState) -> (Vec<Action>, Vec<Asked>) {
    sweep_with(state, Shown::with(HELD, true))
}

/// What is written on the tab with the strip's menu open.
pub(super) fn menu_words(state: &AppState) -> String {
    let mut shown = Shown::with(HELD, true);
    for _ in 0..3 {
        let _ = pass(&mut shown, state, Vec::new());
    }
    shown.words
}

pub(super) fn lines_of(words: &str) -> Vec<&str> {
    words.lines().collect()
}

/// Where the first line that `is` what is wanted stands among the lines written. The test
/// fails if there is none.
pub(super) fn at(lines: &[&str], is: impl Fn(&str) -> bool) -> usize {
    let found = lines.iter().position(|line| is(line));
    found.expect("the line is written")
}

#[test]
fn with_no_labels_open_the_button_and_what_it_will_do_are_the_top_of_the_tab() {
    let state = able();
    let (actions, _) = swept(&state);
    assert!(
        actions.contains(&label_scan()),
        "the button asks to label, with what the window holds of its card: {actions:?}"
    );
    let said = written(&state);
    let lines = lines_of(&said);
    let button = at(&lines, |line| line == "Label this scan");
    let line = at(&lines, |line| line == what_will_run(&state));
    let first = at(&lines, |line| line.contains("Labels are parts"));
    assert!(button < line && line < first, "in that order: {said}");
    assert!(lines.contains(&"Options"), "{said}");
    // What labels are is true of the labels Volsa makes as of those another program made.
    assert_eq!(
        lines[first],
        "Labels are parts of the head outlined on this scan: by Volsa, when it labels the scan, \
         or by another program, as a NIfTI file (.nii or .nii.gz) with a table of names beside \
         it if it has one."
    );
}

#[test]
fn where_labelling_cannot_run_the_button_is_greyed_and_the_sentence_says_why() {
    let mut reading = able();
    let open = Action::OpenLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    };
    reduce(&mut reading, open);
    let mut missing = able();
    missing.labelling.ready = Ready::No("Made-up: the runtime files are not there.".to_owned());
    let unchecked = thick_scan_ready();
    for state in [missing, unchecked, reading] {
        let why = can_label(&state).expect_err("labelling cannot run here");
        let (actions, _) = swept(&state);
        assert!(!asks_to_label(&actions), "{why}: {actions:?}");
        let said = written(&state);
        assert!(said.contains("Label this scan"), "{said}");
        assert!(lines_of(&said).contains(&why.as_str()), "{why}: {said}");
        assert!(
            !said.contains(&what_will_run(&state)),
            "what will run is not said where nothing can: {said}"
        );
    }
}

#[test]
fn while_a_run_is_on_the_strip_says_the_stage_and_offers_stop_and_not_the_button() {
    let mut state = able();
    state.labelling.running = Some(Stage::Finding { step: 2, of: 3 });
    state.labelling.on = state.scene.series.clone();
    let (actions, _) = swept(&state);
    assert!(
        actions.contains(&act(LabellingAction::StopLabelling)),
        "{actions:?}"
    );
    assert!(!asks_to_label(&actions), "{actions:?}");
    let said = written(&state);
    let lines = lines_of(&said);
    for line in [
        "Finding the structures, 2 of 3",
        "Stop",
        "The 3D view may be slow while the graphics card is busy.",
    ] {
        assert!(lines.contains(&line), "{line}: {said}");
    }
    assert!(!said.contains("Label this scan"), "{said}");
}

#[test]
fn each_stage_is_said_in_plain_words() {
    let said = stage_in_words;
    assert_eq!(said(Stage::Preparing), "Preparing the scan");
    let finding = Stage::Finding { step: 1, of: 2 };
    assert_eq!(said(finding), "Finding the structures, 1 of 2");
    assert_eq!(said(Stage::Cortex), "Dividing the cortex");
    assert_eq!(said(Stage::Checking), "The model is checking its work");
    assert_eq!(said(Stage::Tidying), "Tidying up");
}

#[test]
fn a_result_kept_for_the_series_is_said_with_show_make_again_and_delete() {
    let state = kept_with_labels_open();
    let (actions, _) = swept(&state);
    for wanted in [
        act(LabellingAction::ShowKept),
        act(LabellingAction::DeleteKept),
        label_scan(),
    ] {
        assert!(actions.contains(&wanted), "{wanted:?}: {actions:?}");
    }
    let said = written(&state);
    let lines = lines_of(&said);
    assert!(
        lines.contains(&"Labels made on 2099-01-02 are kept for this series."),
        "{said}"
    );
    for button in ["Show", "Make again", "Delete"] {
        assert!(lines.contains(&button), "{button}: {said}");
    }
    // With labels open the button that labels is not there: only "Make again" asks to.
    assert!(!said.contains("Label this scan"), "{said}");
    let mut with_none_open = able();
    with_none_open.labelling.kept = state.labelling.kept.clone();
    with_none_open.labelling.kept_files = state.labelling.kept_files.clone();
    let said = written(&with_none_open);
    assert!(said.contains("Label this scan") && said.contains("kept for this series"));
}

#[test]
fn make_again_is_greyed_where_labelling_cannot_run_and_the_other_two_still_work() {
    let mut state = kept_with_labels_open();
    state.labelling.ready = Ready::No("Made-up: the runtime files are not there.".to_owned());
    let (actions, _) = swept(&state);
    assert!(!asks_to_label(&actions), "{actions:?}");
    assert!(
        actions.contains(&act(LabellingAction::ShowKept)),
        "{actions:?}"
    );
    assert!(
        actions.contains(&act(LabellingAction::DeleteKept)),
        "{actions:?}"
    );
}

#[test]
fn labels_from_a_file_leave_the_tab_as_it_was_and_the_strip_says_nothing() {
    let mut state = named();
    state.labelling.ready = Ready::Yes;
    state.labelling.cards = cards();
    let said = written(&state);
    for absent in ["Label this scan", "Options", "kept for this series", "Stop"] {
        assert!(!said.contains(absent), "{absent}: {said}");
    }
    assert!(said.contains(MADE_ELSEWHERE), "{said}");
    assert!(!said.contains("Not for diagnostic use"), "{said}");
    let (actions, asked) = swept(&state);
    assert!(!asks_to_label(&actions), "{actions:?}");
    assert!(
        !asked
            .iter()
            .any(|asked| matches!(asked, Asked::Labelling(_))),
        "{asked:?}"
    );
}
