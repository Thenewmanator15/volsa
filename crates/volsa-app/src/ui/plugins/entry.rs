//! The section "Added by you": the button that adds a plugin, what the workers are doing for
//! plugins while they are, and an entry for each plugin added, with what it is, where it came
//! from, what it accepts and gives back, where it runs, how it stands, and its buttons.

use egui::{Button, RichText, ScrollArea, Ui};

use super::{named, origin, quiet};
use crate::action::Action;
use crate::plugins::{PluginAction, ToolEntry};
use crate::ui::theme;

/// Said when a person points at the button that adds a plugin.
const ADD_HINT: &str = "Choose the folder of a model someone else made, with the plugin.json \
                        that tells Volsa how to run it";
/// Said when a person points at the button that stops what the workers are doing for plugins.
const STOP_HINT: &str = "Stop adding the plugin; nothing of it is kept";
/// Said when a person points at the button that runs a plugin.
const RUN_HINT: &str = "Run this plugin on the scan in the 3D view";
/// Said when a person points at the button that opens a plugin's result.
const SHOW_HINT: &str = "Open the labels this plugin made for this scan";
/// Said when a person points at the button that deletes a plugin's result.
const DELETE_HINT: &str = "Delete the labels this plugin made for this scan";
/// Said when a person points at the button that removes a plugin.
const REMOVE_HINT: &str = "Remove this plugin from Volsa; you are asked first";
/// The height the entries leave beneath them, in points, before they scroll: room for
/// whatever the tab holds beneath them.
const ROOM_BENEATH: f32 = 140.0;
/// The least height the entries are given, in points, however short the tab.
const SHORTEST: f32 = 24.0;

/// Draw the section: its heading and the button that adds a plugin, which waits while the
/// workers are `busy` with plugins, saying what they are doing; then an entry for each plugin.
/// `held` is how many bytes of the graphics card the window draws on it holds, as a run is
/// told. Pushes what the entries' buttons ask for, and gives whether "Add a plugin…" was
/// pressed, which needs a folder chosen.
pub fn plugin_section(
    ui: &mut Ui,
    entries: &[ToolEntry],
    busy: Option<&str>,
    held: u64,
    actions: &mut Vec<Action>,
) -> bool {
    let add = ui
        .horizontal(|ui| {
            ui.label(RichText::new("Added by you").small().strong());
            let button = Button::new("Add a plugin…").small();
            let button = ui.add_enabled(busy.is_none(), button);
            button.on_hover_text(ADD_HINT).clicked()
        })
        .inner;
    if let Some(busy) = busy {
        ui.horizontal_wrapped(|ui| {
            ui.label(quiet(busy));
            let stop = ui.small_button("Stop").on_hover_text(STOP_HINT);
            if stop.clicked() {
                actions.push(Action::Plugins(PluginAction::Stop));
            }
        });
    }
    if !entries.is_empty() {
        // The entries scroll rather than push what is beneath them out of a short tab.
        let tallest = (ui.available_height() - ROOM_BENEATH).max(SHORTEST);
        let scrolled = ScrollArea::vertical()
            .id_salt("plugins added")
            .min_scrolled_height(SHORTEST)
            .max_height(tallest);
        scrolled.show(ui, |ui| {
            for entry in entries {
                one(ui, entry, held, actions);
            }
        });
    }
    ui.separator();
    add
}

/// One plugin's entry: its name, how it stands and its buttons first, where they are seen
/// without scrolling; then who made it, where it came from, and what it does.
fn one(ui: &mut Ui, tool: &ToolEntry, held: u64, actions: &mut Vec<Action>) {
    let entry = &tool.entry;
    ui.label(RichText::new(named(&entry.name, &entry.version)).strong());
    let colour = if entry.checked.is_ok() {
        theme::QUIET_TEXT
    } else {
        theme::WARNING
    };
    ui.label(RichText::new(&tool.state).small().color(colour));
    ui.horizontal_wrapped(|ui| buttons(ui, tool, held, actions));
    if entry.checked.is_ok() {
        ui.label(quiet(&origin(
            &entry.made_by,
            &entry.source,
            &entry.licence,
        )));
        let what = format!(
            "For {}. {} Runs on: {}.",
            entry.accepts_words, entry.returns, entry.runs_on
        );
        ui.label(quiet(&what));
    }
}

/// An entry's buttons: Run, or Stop while it runs; its result shown or deleted; and Remove. A
/// plugin whose files failed their check can only be removed.
fn buttons(ui: &mut Ui, tool: &ToolEntry, held: u64, actions: &mut Vec<Action>) {
    let id = &tool.entry.id;
    let mut pressed = |label: &str, hint: &str, action: PluginAction| {
        if ui.small_button(label).on_hover_text(hint).clicked() {
            actions.push(Action::Plugins(action));
        }
    };
    if tool.entry.checked.is_ok() {
        if tool.running {
            pressed("Stop", "Stop this plugin's run", PluginAction::Stop);
        } else {
            let run = PluginAction::Run {
                id: id.clone(),
                held,
            };
            pressed("Run", RUN_HINT, run);
        }
        pressed("Show result", SHOW_HINT, PluginAction::ShowKept(id.clone()));
        let delete = PluginAction::DeleteKept(id.clone());
        pressed("Delete result", DELETE_HINT, delete);
    }
    pressed("Remove", REMOVE_HINT, PluginAction::Remove(id.clone()));
}
