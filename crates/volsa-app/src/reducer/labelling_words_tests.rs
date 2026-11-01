//! Labelling in the state: the words that say why a run cannot start and what will run.

use volsa_models::{CardId, CardKind, RunsOn};
use volsa_segment::ModelChoice;

use super::labelling_tests::{
    able, card, cards, looked, looked_on, reading_labels, running, slices_apart,
};
use super::tests::showing;
use super::*;
use crate::labelling::{LabellingAction, Ready, can_label, what_will_run};
use crate::testing::{THICK_SCAN, thick_scan_ready};

#[test]
fn it_says_in_a_sentence_why_a_scan_cannot_be_labelled_yet() {
    assert_eq!(can_label(&able()), Ok(()));
    let nothing_open = {
        let mut state = AppState::default();
        looked(&mut state, Ready::Yes);
        state
    };
    let mut building = showing(THICK_SCAN);
    looked(&mut building, Ready::Yes);
    let mut not_possible = thick_scan_ready();
    looked(
        &mut not_possible,
        Ready::No("No models were found.".to_owned()),
    );
    let cases = [
        (
            thick_scan_ready(),
            "Volsa has not yet checked whether this computer can label a scan.",
        ),
        (not_possible, "No models were found."),
        (
            running(),
            "A scan is already being labelled. Wait for it to finish, or stop it.",
        ),
        (
            reading_labels(),
            "A label file is still being opened. Wait for it to finish.",
        ),
        (
            building,
            "The 3D view of this scan is still being built. Labelling can start when it is ready.",
        ),
        (
            nothing_open,
            "Labelling needs a scan with a 3D view. Open a scan that has one first.",
        ),
    ];
    for (state, sentence) in cases {
        assert_eq!(can_label(&state), Err(sentence.to_owned()));
    }
}

fn line(choice: ModelChoice, mm: f64, cortex: bool, runs_on: RunsOn) -> String {
    let mut state = able();
    slices_apart(&mut state, mm);
    let chosen = [
        Action::Labelling(LabellingAction::SetModelChoice(choice)),
        Action::Labelling(LabellingAction::SetCortex(cortex)),
        Action::Labelling(LabellingAction::SetRunsOn(runs_on)),
    ];
    for action in chosen {
        reduce(&mut state, action);
    }
    what_will_run(&state)
}

#[test]
fn left_to_volsa_the_line_says_which_model_was_picked_and_why() {
    let card_a = "the graphics card (Made-up card A)";
    let thin = "Standard was picked because the slices are no more than 3 mm apart.";
    let thick = "Robust was picked because the slices are more than 3 mm apart.";
    let cases = [
        (1.0, true, RunsOn::Automatic, "Standard", card_a, thin),
        (6.0, true, RunsOn::Automatic, "Robust", card_a, thick),
        (
            3.0,
            false,
            RunsOn::Processor,
            "Standard",
            "the processor",
            thin,
        ),
    ];
    for (mm, cortex, runs_on, model, place, reason) in cases {
        let with = if cortex { " with cortex areas" } else { "" };
        let said = format!("{model} model{with}, on {place}. {reason}");
        let line = line(ModelChoice::Automatic, mm, cortex, runs_on);
        assert_eq!(line, said, "{mm} mm, {runs_on:?}");
    }
}

#[test]
fn chosen_by_hand_the_line_says_the_model_and_the_place_and_nothing_of_the_slices() {
    use ModelChoice::{Robust, Standard};
    let cases = [
        (Standard, 6.0, true, RunsOn::Card(CardId(3))),
        (Robust, 1.0, false, RunsOn::Processor),
        (Robust, 1.0, false, RunsOn::Card(CardId(1))),
        (Standard, 1.0, false, RunsOn::Card(CardId(9))),
    ];
    let said = [
        "Standard model with cortex areas, on the graphics card (Made-up card B).",
        "Robust model, on the processor.",
        "Robust model, on the graphics card (Made-up shared graphics).",
        "Standard model, on the graphics card you chose (it is not listed now).",
    ];
    for ((choice, mm, cortex, runs_on), sentence) in cases.into_iter().zip(said) {
        let line = line(choice, mm, cortex, runs_on);
        assert_eq!(line, sentence, "{choice:?}, {mm} mm, {runs_on:?}");
    }
}

#[test]
fn automatic_names_the_first_discrete_card_and_otherwise_the_processor() {
    let mut state = able();
    slices_apart(&mut state, 1.0);
    let by_hand = cards().into_iter().rev().collect();
    looked_on(&mut state, Ready::Yes, by_hand);
    assert!(what_will_run(&state).contains("on the graphics card (Made-up card B)."));
    let not_discrete = vec![
        card(1, "Made-up shared graphics", CardKind::Integrated),
        card(4, "Made-up software", CardKind::Software),
    ];
    for cards in [not_discrete, Vec::new()] {
        looked_on(&mut state, Ready::Yes, cards);
        assert!(
            what_will_run(&state).contains("on the processor."),
            "{}",
            what_will_run(&state)
        );
    }
}

#[test]
fn with_nothing_to_go_by_automatic_does_not_say_which_model_will_run() {
    let mut state = AppState::default();
    looked(&mut state, Ready::Yes);
    assert_eq!(
        what_will_run(&state),
        "Standard or robust model with cortex areas, on the graphics card (Made-up card A). \
         Which one runs is picked from how far apart the slices are."
    );
}
