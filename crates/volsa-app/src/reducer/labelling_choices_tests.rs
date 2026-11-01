//! Labelling in the state: what is chosen is asked to be kept as it is chosen, and a card
//! kept by its name is found again when the cards are listed.

use std::path::PathBuf;

use volsa_models::{CardId, RunsOn};
use volsa_segment::{ModelChoice, Stage};

use super::labelling_tests::{able, cards, label, looked, looked_on};
use super::*;
use crate::choices::{Choices, RunsOnKept};
use crate::labelling::{LabellingAction, Ready};

fn act(state: &mut AppState, action: LabellingAction) -> Vec<Effect> {
    reduce(state, Action::Labelling(action))
}

/// Choices that keep a card, by this name, and are otherwise the usual.
fn on_card(name: &str) -> Choices {
    Choices {
        runs_on: RunsOnKept::Card(name.to_owned()),
        ..Choices::default()
    }
}

/// A state as Volsa starts with these choices kept from the run before, and the cards looked
/// for.
fn started_with(choices: Choices) -> AppState {
    let mut state = AppState::default();
    choices.put_in(&mut state.labelling);
    act(&mut state, LabellingAction::Look);
    state
}

#[test]
fn each_choice_is_asked_to_be_kept_as_it_is_made() {
    let mut state = AppState::default();
    let mut kept = Choices::default();
    let chosen = [
        LabellingAction::SetModelChoice(ModelChoice::Standard),
        LabellingAction::SetCortex(false),
        LabellingAction::SetRunsOn(RunsOn::Processor),
        LabellingAction::SetKeep(false),
    ];
    for action in chosen {
        match &action {
            LabellingAction::SetModelChoice(choice) => kept.choice = *choice,
            LabellingAction::SetCortex(cortex) => kept.cortex = *cortex,
            LabellingAction::SetRunsOn(_) => kept.runs_on = RunsOnKept::Processor,
            LabellingAction::SetKeep(keep) => kept.keep = *keep,
            _ => {}
        }
        assert_eq!(
            act(&mut state, action),
            [Effect::StoreChoices(kept.clone())]
        );
    }
    assert_eq!(Choices::of(&state.labelling), kept);
}

#[test]
fn a_folder_is_kept_and_looked_at_and_what_is_not_a_choice_is_not_kept() {
    let mut state = AppState::default();
    let models = PathBuf::from("made-up models");
    let effects = act(&mut state, LabellingAction::SetModelsFolder(models.clone()));
    let kept = Choices {
        models: Some(models.clone()),
        ..Choices::default()
    };
    let look = Effect::LookForLabelling {
        look: 1,
        models: Some(models),
        runtime: None,
    };
    assert_eq!(effects, [Effect::StoreChoices(kept), look]);
    let runtime = PathBuf::from("made-up runtime");
    let effects = act(
        &mut state,
        LabellingAction::SetRuntimeFolder(runtime.clone()),
    );
    assert!(effects.contains(&Effect::StoreChoices(Choices::of(&state.labelling))));
    assert_eq!(state.labelling.runtime, Some(runtime));
    let drawing = LabellingAction::DrawingOn("Made-up card A".to_owned());
    assert_eq!(
        act(&mut state, drawing),
        [],
        "the card drawn on is no choice"
    );
}

#[test]
fn a_card_chosen_is_asked_to_be_kept_by_its_name() {
    let mut state = able();
    let chosen = LabellingAction::SetRunsOn(RunsOn::Card(CardId(3)));
    let effects = act(&mut state, chosen);
    assert_eq!(effects, [Effect::StoreChoices(on_card("Made-up card B"))]);
}

#[test]
fn a_card_kept_by_its_name_is_found_when_the_cards_are_listed() {
    let mut state = started_with(on_card("Made-up card B"));
    assert_eq!(state.labelling.runs_on, RunsOn::Automatic, "not listed yet");
    looked(&mut state, Ready::Yes);
    assert_eq!(state.labelling.runs_on, RunsOn::Card(CardId(3)));
    assert_eq!(state.labelling.card_kept, None, "it has been looked for");
}

#[test]
fn a_card_kept_by_a_name_no_card_has_now_is_automatic() {
    let mut state = started_with(on_card("Made-up card C"));
    looked(&mut state, Ready::Yes);
    assert_eq!(state.labelling.runs_on, RunsOn::Automatic);
    assert_eq!(state.labelling.card_kept, None);
    // And a computer with no cards at all lists none to find it among.
    on_card("Made-up card B").put_in(&mut state.labelling);
    act(&mut state, LabellingAction::Look);
    looked_on(&mut state, Ready::Yes, Vec::new());
    assert_eq!(state.labelling.runs_on, RunsOn::Automatic);
    assert_eq!(state.labelling.card_kept, None);
}

#[test]
fn what_an_older_look_found_does_not_use_up_the_card_kept() {
    let mut state = started_with(on_card("Made-up card B"));
    let older = LabellingAction::LabellingLooked {
        look: 0,
        ready: Ready::Yes,
        cards: cards(),
    };
    act(&mut state, older);
    assert_eq!(state.labelling.card_kept.as_deref(), Some("Made-up card B"));
    looked(&mut state, Ready::Yes);
    assert_eq!(state.labelling.runs_on, RunsOn::Card(CardId(3)));
}

#[test]
fn a_place_chosen_outright_takes_the_place_of_the_card_kept_and_waiting() {
    let mut state = started_with(on_card("Made-up card B"));
    let effects = act(&mut state, LabellingAction::SetRunsOn(RunsOn::Processor));
    let kept = Choices {
        runs_on: RunsOnKept::Processor,
        ..Choices::default()
    };
    assert_eq!(effects, [Effect::StoreChoices(kept)]);
    looked(&mut state, Ready::Yes);
    assert_eq!(state.labelling.runs_on, RunsOn::Processor);
}

#[test]
fn a_card_still_waiting_is_kept_when_another_choice_is_made() {
    let mut state = started_with(on_card("Made-up card B"));
    let effects = act(&mut state, LabellingAction::SetCortex(false));
    let kept = Choices {
        cortex: false,
        ..on_card("Made-up card B")
    };
    assert_eq!(effects, [Effect::StoreChoices(kept)]);
}

#[test]
fn a_run_asked_for_goes_to_the_card_found_by_its_name() {
    let mut state = able();
    on_card("Made-up card B").put_in(&mut state.labelling);
    act(&mut state, LabellingAction::Look);
    looked(&mut state, Ready::Yes);
    let effects = label(&mut state);
    let Some(Effect::LabelScan { runs_on, .. }) = effects.first() else {
        panic!("a run was asked for: {effects:?}");
    };
    assert_eq!(*runs_on, RunsOn::Card(CardId(3)));
    assert_eq!(state.labelling.running, Some(Stage::Preparing));
}
