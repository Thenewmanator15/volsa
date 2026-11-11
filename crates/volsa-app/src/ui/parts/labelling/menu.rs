//! The menu beside the button that labels: which model, whether the cortex is divided into its
//! areas, where the run goes, whether the result is kept, and where the model and runtime
//! files are.

use egui::{Id, Popup, PopupCloseBehavior, RichText, Ui};
use voxa_models::{Card, CardKind, RunsOn};
use voxa_segment::{ModelChoice, THICK_MM};

use super::super::super::theme;
use super::Wants;
use crate::action::Action;
use crate::labelling::{Labelling, LabellingAction};

/// What the menu is known by.
const MENU: &str = "labelling options";
/// Said beside the entry of a card that shares the computer's memory.
const SLOWER: &str = "usually slower than the processor";
/// Said of the place left to Voxa.
const AUTOMATIC_PLACE: &str =
    "Each graphics card with memory of its own, in turn, then the processor";
/// Said of a folder that has not been chosen.
const NO_FOLDER: &str = "None chosen: Voxa looks where these files are usually kept.";

/// The menu's own identity, for holding it open from outside.
pub fn menu_id() -> Id {
    Id::new(MENU)
}

/// The button that opens the menu, and the menu while it is open. Gives the folder the
/// person asked to choose, if they did.
pub(super) fn options(
    ui: &mut Ui,
    labelling: &Labelling,
    actions: &mut Vec<Action>,
) -> Option<Wants> {
    let button = ui.button("Options");
    let menu = Popup::menu(&button)
        .id(menu_id())
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside);
    let shown = menu.show(|ui| choices(ui, labelling, actions));
    shown.and_then(|shown| shown.inner)
}

/// What the menu holds.
fn choices(ui: &mut Ui, labelling: &Labelling, actions: &mut Vec<Action>) -> Option<Wants> {
    model(ui, labelling, actions);
    let mut cortex = labelling.cortex;
    let divide = ui.checkbox(&mut cortex, "Cortex areas");
    if divide
        .on_hover_text("Divide the cortex into its areas")
        .changed()
    {
        send(actions, LabellingAction::SetCortex(cortex));
    }
    places(ui, labelling, actions);
    let mut keep = labelling.keep;
    let keeps = ui.checkbox(&mut keep, "Keep results on this computer");
    if keeps
        .on_hover_text("Keep each result, to show again when the same series is opened")
        .changed()
    {
        send(actions, LabellingAction::SetKeep(keep));
    }
    folders(ui, labelling)
}

/// Ask for an action about labelling.
fn send(actions: &mut Vec<Action>, action: LabellingAction) {
    actions.push(Action::Labelling(action));
}

/// A line over a group of entries.
fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().color(theme::QUIET_TEXT));
}

/// Which model: left to Voxa, or one of the two.
fn model(ui: &mut Ui, labelling: &Labelling, actions: &mut Vec<Action>) {
    heading(ui, "Model");
    let automatic =
        format!("Robust when the slices are more than {THICK_MM} mm apart, otherwise Standard");
    let ways = [
        (ModelChoice::Automatic, "Automatic", automatic.as_str()),
        (
            ModelChoice::Standard,
            "Standard",
            "The standard model, whatever the scan",
        ),
        (
            ModelChoice::Robust,
            "Robust",
            "The robust model, whatever the scan",
        ),
    ];
    for (way, name, hover) in ways {
        let chosen = labelling.choice == way;
        if ui
            .selectable_label(chosen, name)
            .on_hover_text(hover)
            .clicked()
            && !chosen
        {
            send(actions, LabellingAction::SetModelChoice(way));
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

/// Where the run goes: left to Voxa, a graphics card by its name, or the processor. A card
/// that is software standing in for one is never a place to run, and is not offered. A card
/// the system lists twice is offered once, as the first of its name: a choice is kept by the
/// card's name and found again as the first card of that name.
fn places(ui: &mut Ui, labelling: &Labelling, actions: &mut Vec<Action>) {
    heading(ui, "Runs on");
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
    let named = cards
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

/// The two buttons that ask for a folder of files, each saying which folder was chosen.
fn folders(ui: &mut Ui, labelling: &Labelling) -> Option<Wants> {
    ui.separator();
    let mut wants = None;
    let ways = [
        (
            Wants::ModelsFolder,
            "Folder of the models…",
            &labelling.models,
        ),
        (
            Wants::RuntimeFolder,
            "Folder of the runtime…",
            &labelling.runtime,
        ),
    ];
    for (way, name, chosen) in ways {
        let hover = chosen.as_ref().map_or_else(
            || NO_FOLDER.to_owned(),
            |folder| format!("Chosen: {}", folder.display()),
        );
        if ui.button(name).on_hover_text(hover).clicked() {
            wants = Some(way);
            ui.close();
        }
    }
    wants
}
