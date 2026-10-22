//! The menu beside the button that labels: what it offers, and what each entry asks for.

use voxa_models::{CardId, CardKind, RunsOn};
use voxa_segment::ModelChoice;

use super::super::Asked;
use super::tests::{able, act, card, lines_of, menu_words, swept_menu};
use super::*;
use crate::labelling::LabellingAction;

#[test]
fn a_card_the_system_lists_twice_is_offered_once_as_the_first_of_its_name() {
    let mut state = able();
    let again = card(4, "Made-up card A", CardKind::Discrete);
    state.labelling.cards.push(again.clone());
    let said = menu_words(&state);
    let entries = lines_of(&said);
    let named = entries.iter().filter(|line| **line == "Made-up card A");
    assert_eq!(named.count(), 1, "{said}");
    // A choice is kept by the card's name and found again as the first card of that name.
    let (actions, _) = swept_menu(&state);
    let first = act(LabellingAction::SetRunsOn(RunsOn::Card(CardId(2))));
    assert!(actions.contains(&first), "{actions:?}");
    let second = act(LabellingAction::SetRunsOn(RunsOn::Card(again.id)));
    assert!(!actions.contains(&second), "{actions:?}");
}

#[test]
fn the_menu_gives_each_choice_it_offers_other_than_the_one_in_force() {
    let (actions, _) = swept_menu(&able());
    let expected = [
        LabellingAction::SetModelChoice(ModelChoice::Standard),
        LabellingAction::SetModelChoice(ModelChoice::Robust),
        LabellingAction::SetCortex(false),
        LabellingAction::SetRunsOn(RunsOn::Card(CardId(1))),
        LabellingAction::SetRunsOn(RunsOn::Card(CardId(2))),
        LabellingAction::SetRunsOn(RunsOn::Processor),
        LabellingAction::SetKeep(false),
    ];
    for wanted in expected {
        assert!(
            actions.contains(&act(wanted.clone())),
            "{wanted:?}: {actions:?}"
        );
    }
    let in_force = [
        LabellingAction::SetModelChoice(ModelChoice::Automatic),
        LabellingAction::SetRunsOn(RunsOn::Automatic),
        LabellingAction::SetRunsOn(RunsOn::Card(CardId(3))),
    ];
    for not_wanted in in_force {
        assert!(
            !actions.contains(&act(not_wanted.clone())),
            "{not_wanted:?}"
        );
    }
    let mut other = able();
    other.labelling.choice = ModelChoice::Robust;
    other.labelling.cortex = false;
    other.labelling.runs_on = RunsOn::Processor;
    other.labelling.keep = false;
    let (actions, _) = swept_menu(&other);
    let expected = [
        LabellingAction::SetModelChoice(ModelChoice::Automatic),
        LabellingAction::SetModelChoice(ModelChoice::Standard),
        LabellingAction::SetCortex(true),
        LabellingAction::SetRunsOn(RunsOn::Automatic),
        LabellingAction::SetKeep(true),
    ];
    for wanted in expected {
        assert!(
            actions.contains(&act(wanted.clone())),
            "{wanted:?}: {actions:?}"
        );
    }
}

#[test]
fn the_menu_names_every_card_but_a_software_one_and_warns_of_an_integrated_one() {
    let said = menu_words(&able());
    let lines = lines_of(&said);
    let warning = "usually slower than the processor";
    let shared = lines
        .iter()
        .filter(|line| line.contains("Made-up shared graphics"));
    let shared: Vec<&&str> = shared.collect();
    assert!(
        shared.len() == 1 && shared[0].contains(warning),
        "the integrated card's entry carries its warning: {said}"
    );
    let discrete = lines.iter().find(|line| line.contains("Made-up card A"));
    assert!(
        discrete.is_some_and(|line| !line.contains(warning)),
        "{said}"
    );
    assert_eq!(
        said.matches(warning).count(),
        1,
        "only the integrated one: {said}"
    );
    assert!(!said.contains("Made-up software renderer"), "{said}");
    for entry in [
        "Automatic",
        "Standard",
        "Robust",
        "Cortex areas",
        "Runs on",
        "Processor",
        "Keep results on this computer",
    ] {
        assert!(lines.contains(&entry), "{entry}: {said}");
    }
}

#[test]
fn the_menu_asks_for_the_folder_of_the_models_and_the_folder_of_the_runtime() {
    let (_, asked) = swept_menu(&able());
    assert!(
        asked.contains(&Asked::Labelling(Wants::ModelsFolder)),
        "{asked:?}"
    );
    assert!(
        asked.contains(&Asked::Labelling(Wants::RuntimeFolder)),
        "{asked:?}"
    );
    assert!(
        !asked.contains(&Asked::Labelling(Wants::SaveLabels)),
        "{asked:?}"
    );
}

#[test]
fn where_the_window_has_no_graphics_device_it_holds_nothing_of_a_card() {
    assert_eq!(held_on_card(&able(), None), 0);
    assert_eq!(held_on_card(&AppState::default(), None), 0);
}
