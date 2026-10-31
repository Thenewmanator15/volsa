//! The strip in the Tools tab: what it shows in each state, and what each of its buttons and
//! menu entries asks for, found by clicking every button of the tab.

use std::path::PathBuf;

use egui::Ui;
use voxa_labels::{Made, Verdict};
use voxa_segment::Stage;

use super::*;
use crate::labelling::{LabellingAction, Ready, can_label, what_will_run};
use crate::labels::NamesFrom;
use crate::reducer::reduce;
use crate::testing::thick_scan_ready;
use crate::ui::commands::{Confirm, Request};
use crate::ui::parts::Asked;
use crate::ui::tabs::made_up::{
    able, cards, labelled_by, named, once, screen, sweep_tools, tools_written,
};

/// Who made labels: Voxa, with a model, on a card, on a day, and two notes.
pub(super) fn by_voxa() -> Made {
    Made {
        by: "Voxa 9.8.7".to_owned(),
        model: "Made-up model, robust".to_owned(),
        model_sum: String::new(),
        device: "Made-up card A".to_owned(),
        when: "2099-01-02".to_owned(),
        notes: vec![
            "A first made-up note.".to_owned(),
            "A second made-up note.".to_owned(),
        ],
    }
}

/// Labels of the scene's study made by Voxa, open, on a computer that can label.
pub(super) fn made_by_voxa(verdict: Option<Verdict>) -> AppState {
    let mut state = labelled_by(verdict, NamesFrom::Table, Some(by_voxa()));
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

/// Whether what was asked of the window includes labelling the scan.
pub(super) fn asks_to_label(requests: &[Request]) -> bool {
    requests.contains(&Request::LabelScan)
}

/// Click every button of the Tools tab with the strip's menu held open.
pub(super) fn swept_menu(state: &AppState) -> (Vec<Action>, Vec<Request>) {
    let (hand, mut actions, mut requests) = (screen(), Vec::new(), Vec::new());
    let mut draw = |ui: &mut Ui| {
        egui::Popup::open_id(ui.ctx(), menu_id());
        requests.extend(crate::ui::tabs::made_up::tools_tab()(
            ui,
            state,
            &mut actions,
        ));
    };
    // The menu is there to be clicked in from the second pass on.
    let _ = hand.pass(Vec::new(), &mut draw);
    hand.click_each(&mut draw);
    (once(actions), once(requests))
}

/// What is written on the Tools tab with the strip's menu open.
pub(super) fn menu_words(state: &AppState) -> String {
    let hand = screen();
    let mut draw = |ui: &mut Ui| {
        egui::Popup::open_id(ui.ctx(), menu_id());
        let _ = crate::ui::tabs::made_up::tools_tab()(ui, state, &mut Vec::new());
    };
    let mut seen = hand.pass(Vec::new(), &mut draw);
    for _ in 0..2 {
        seen = hand.pass(Vec::new(), &mut draw);
    }
    seen.words.join("\n")
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
fn with_no_labels_open_the_button_and_what_it_will_do_are_the_top_of_the_tool() {
    let state = able();
    let (actions, requests) = sweep_tools(&state);
    assert!(
        asks_to_label(&requests),
        "the button asks the window to label: {requests:?}"
    );
    let itself =
        |action: &Action| matches!(action, Action::Labelling(LabellingAction::LabelScan { .. }));
    assert!(!actions.iter().any(itself), "and does not label it itself");
    let said = tools_written(&state);
    let lines = lines_of(&said);
    let tool = at(&lines, |line| line == "Label this brain scan");
    let button = at(&lines, |line| line == "Label this scan");
    let line = at(&lines, |line| line == what_will_run(&state));
    assert!(tool < button && button < line, "in that order: {said}");
    assert!(lines.contains(&"Options"), "{said}");
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
        let (_, requests) = sweep_tools(&state);
        assert!(!asks_to_label(&requests), "{why}: {requests:?}");
        let said = tools_written(&state);
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
    let (actions, requests) = sweep_tools(&state);
    assert!(
        actions.contains(&act(LabellingAction::StopLabelling)),
        "{actions:?}"
    );
    assert!(!asks_to_label(&requests), "{requests:?}");
    let said = tools_written(&state);
    let lines = lines_of(&said);
    for line in [
        "Label this brain scan",
        "Locating the structures, 2 of 3",
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
    assert_eq!(said(finding), "Locating the structures, 1 of 2");
    assert_eq!(said(Stage::Cortex), "Dividing the cortex");
    assert_eq!(said(Stage::Checking), "The model is checking its work");
    assert_eq!(said(Stage::Tidying), "Tidying up");
}

#[test]
fn a_result_kept_for_the_series_is_said_with_show_make_again_and_delete() {
    let state = kept_with_labels_open();
    let (actions, requests) = sweep_tools(&state);
    let show = act(LabellingAction::ShowKept);
    assert!(actions.contains(&show), "{show:?}: {actions:?}");
    // Deleting what took minutes to make asks first, and does nothing until it is answered.
    let delete = act(LabellingAction::DeleteKept);
    assert!(!actions.contains(&delete), "{actions:?}");
    let asked = Request::Confirm(Confirm::DeleteKept);
    assert!(requests.contains(&asked), "Delete: {requests:?}");
    assert!(asks_to_label(&requests), "Make again: {requests:?}");
    let said = tools_written(&state);
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
    let said = tools_written(&with_none_open);
    assert!(said.contains("Label this scan") && said.contains("kept for this series"));
}

#[test]
fn make_again_is_greyed_where_labelling_cannot_run_and_the_other_two_still_work() {
    let mut state = kept_with_labels_open();
    state.labelling.ready = Ready::No("Made-up: the runtime files are not there.".to_owned());
    let (actions, requests) = sweep_tools(&state);
    assert!(!asks_to_label(&requests), "{requests:?}");
    assert!(
        actions.contains(&act(LabellingAction::ShowKept)),
        "{actions:?}"
    );
    let asked = Request::Confirm(Confirm::DeleteKept);
    assert!(requests.contains(&asked), "{requests:?}");
}

#[test]
fn labels_from_a_file_leave_the_tool_with_nothing_to_offer_and_it_says_why() {
    let mut state = named();
    state.labelling.ready = Ready::Yes;
    state.labelling.cards = cards();
    let said = tools_written(&state);
    for absent in ["Label this scan", "Options", "kept for this series", "Stop"] {
        assert!(!said.contains(absent), "{absent}: {said}");
    }
    assert!(
        said.contains("Label this brain scan"),
        "still listed: {said}"
    );
    assert!(
        said.contains(
            "Labels are open on this series. Close them in the Labels tab to label it again."
        ),
        "{said}"
    );
    assert!(!said.contains("Not for diagnostic use"), "{said}");
    let (_, requests) = sweep_tools(&state);
    assert!(!asks_to_label(&requests), "{requests:?}");
    assert!(
        !requests
            .iter()
            .any(|asked| matches!(asked, Request::Choose(Asked::Labelling(_)))),
        "{requests:?}"
    );
}

#[test]
fn once_asked_the_window_labels_the_scan_told_what_it_holds_of_the_card() {
    // Where there is no graphics device the window holds nothing of a card.
    let held = act(LabellingAction::LabelScan { held: 0 });
    assert_eq!(label_when_asked(&able(), None), held);
}
