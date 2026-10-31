//! The menu beside the button that labels: which model, whether the cortex is divided into its
//! areas, where the run goes, whether the result is kept, and where the model and runtime
//! files are. Every entry but the places a run can go is an action of the list of actions.

use egui::{Id, Popup, PopupCloseBehavior, RichText, Ui};
use voxa_models::{Card, CardKind, RunsOn};
use voxa_segment::ModelChoice;

use super::super::super::theme;
use crate::action::Action;
use crate::labelling::{Labelling, LabellingAction};
use crate::state::AppState;
use crate::ui::commands::{self, Request, menus};

/// What the menu is known by.
const MENU: &str = "labelling options";
/// Said beside the entry of a card that shares the computer's memory.
const SLOWER: &str = "usually slower than the processor";
/// Said of the place left to Voxa.
const AUTOMATIC_PLACE: &str =
    "Each graphics card with memory of its own, in turn, then the processor";
/// Said of a folder that has not been chosen.
const NO_FOLDER: &str = "None chosen: Voxa looks where these files are usually kept.";

/// The models to choose between, by the action that chooses each.
const MODELS: [(ModelChoice, &str); 3] = [
    (ModelChoice::Automatic, "tools.model.automatic"),
    (ModelChoice::Standard, "tools.model.standard"),
    (ModelChoice::Robust, "tools.model.robust"),
];

/// The menu's own identity, for holding it open from outside.
pub fn menu_id() -> Id {
    Id::new(MENU)
}

/// The button that opens the menu, and the menu while it is open. What its entries ask of the
/// window goes onto `asked`.
pub(super) fn options(
    ui: &mut Ui,
    state: &AppState,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    let button = ui.button(menus::OPTIONS.label);
    let button = commands::named(ui, &menus::OPTIONS, menus::OPTIONS.label, button);
    let menu = Popup::menu(&button)
        .id(menu_id())
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside);
    menu.show(|ui| choices(ui, state, actions, asked));
}

/// What the menu holds.
fn choices(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    let labelling = &state.labelling;
    model(ui, state, actions, asked);
    commands::offer(
        ui,
        "tools.label.cortex",
        state,
        Some(labelling.cortex),
        actions,
        asked,
    );
    places(ui, labelling, actions);
    commands::offer(
        ui,
        "tools.label.keep",
        state,
        Some(labelling.keep),
        actions,
        asked,
    );
    folders(ui, state, actions, asked);
}

/// Ask for an action about labelling.
fn send(actions: &mut Vec<Action>, action: LabellingAction) {
    actions.push(Action::Labelling(action));
}

/// A line over a group of entries.
fn heading(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .small()
            .color(theme::palette(ui.ctx()).quiet_text),
    );
}

/// Which model: left to Voxa, or one of the two. The one in force is not asked for again.
fn model(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    heading(ui, "Model");
    for (way, id) in MODELS {
        let chosen = state.labelling.choice == way;
        if let Some(does) = commands::button(ui, id, state, Some(chosen)).filter(|_| !chosen) {
            asked.extend(does.run(state, actions));
        }
    }
}

/// What a card is called in the menu: its name, and for one that shares the computer's memory
/// that it is usually slower than the processor.
fn card_entry(card: &Card) -> String {
    match card.kind {
        CardKind::Integrated => format!("{} ({SLOWER})", card.name),
        _ => card.name.clone(),
    }
}

/// The cards the menu offers as places to run: a card that is software standing in for one is
/// never a place to run, and is not offered. A card the system lists twice is offered once, as
/// the first of its name: a choice is kept by the card's name and found again as the first card
/// of that name.
fn real_cards(labelling: &Labelling) -> Vec<&Card> {
    let real = labelling
        .cards
        .iter()
        .filter(|card| card.kind != CardKind::Software);
    let mut cards: Vec<&Card> = Vec::new();
    for card in real {
        if !cards.iter().any(|seen| seen.name == card.name) {
            cards.push(card);
        }
    }
    cards
}

/// What each place a run can go is called in the menu, in the order they are listed.
#[cfg(test)]
pub fn places_named(labelling: &Labelling) -> Vec<String> {
    let cards = real_cards(labelling).into_iter().map(card_entry);
    let automatic = std::iter::once("Automatic".to_owned());
    automatic
        .chain(cards)
        .chain(["Processor".to_owned()])
        .collect()
}

/// Where the run goes: left to Voxa, a graphics card by its name, or the processor. There is a
/// card for each the system lists, so these are rows of a list, as the saved views are, and not
/// actions of the list of actions.
fn places(ui: &mut Ui, labelling: &Labelling, actions: &mut Vec<Action>) {
    heading(ui, "Runs on");
    let named = real_cards(labelling)
        .into_iter()
        .map(|card| (RunsOn::Card(card.id), card_entry(card)));
    let automatic = (RunsOn::Automatic, "Automatic".to_owned());
    let processor = (RunsOn::Processor, "Processor".to_owned());
    let entries = std::iter::once(automatic).chain(named).chain([processor]);
    for (place, name) in entries {
        let chosen = labelling.runs_on == place;
        let mut entry = ui.selectable_label(chosen, name);
        if place == RunsOn::Automatic {
            entry = entry.on_hover_text(AUTOMATIC_PLACE);
        }
        if entry.clicked() && !chosen {
            send(actions, LabellingAction::SetRunsOn(place));
        }
    }
}

/// The two buttons that ask for a folder of files, each followed by which folder was chosen.
fn folders(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    ui.separator();
    let labelling = &state.labelling;
    let ways = [
        ("tools.label.models_folder", &labelling.models),
        ("tools.label.runtime_folder", &labelling.runtime),
    ];
    for (id, chosen) in ways {
        let before = asked.len();
        commands::offer(ui, id, state, None, actions, asked);
        if asked.len() > before {
            ui.close();
        }
        if let Some(folder) = chosen {
            heading(ui, &format!("Chosen: {}", folder.display()));
        }
    }
    if labelling.models.is_none() && labelling.runtime.is_none() {
        heading(ui, NO_FOLDER);
    }
}
