//! The measurements window: the tools, undo and redo, the list of what has been measured on
//! the series in the scene, and saving it.

use egui::{Align, Button, Layout, RichText, TextEdit, Ui, pos2};
use voxa_core::Point;

use super::scene::export::said;
use super::theme;
use crate::action::Action;
use crate::annotations::{Annotation, MAX_LABEL, Tool, measured};
use crate::export::write;
use crate::measurements::{MEASUREMENTS_NAME, measurements_csv, measurements_json, rows};
use crate::planes::grid;
use crate::state::{AppState, Notice};

/// What is said in the window while there is no volume to measure on.
pub const NO_VOLUME: &str = "Measuring needs a 3D view of the series.";
/// How wide the window's contents are, in points.
const WIDTH: f32 = 380.0;
/// How wide a name is in the list, in points.
const NAME_WIDTH: f32 = 120.0;
/// Where the window first opens: at the left, beneath the bars, over the list of series.
const OPENS_AT: [f32; 2] = [12.0, 150.0];

/// Each tool with the word on its button and what the button says when pointed at.
const TOOLS: [(Tool, &str, &str); 5] = [
    (
        Tool::Navigate,
        "Move",
        "The main button moves the position, or turns the 3D view",
    ),
    (
        Tool::Ruler,
        "Ruler",
        "A length: press at one end and let go at the other; in 3D, click two surfaces",
    ),
    (
        Tool::Angle,
        "Angle",
        "An angle, in a plane view: click an arm's end, the corner, the other arm's end",
    ),
    (
        Tool::Region,
        "Region",
        "The scan values in an ellipse, in a plane view: drag from its middle",
    ),
    (Tool::Marker, "Marker", "A named point: click where it goes"),
];

/// Which kind of file the measurements are to be saved as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Save {
    /// A table any spreadsheet opens.
    Csv,
    /// JSON, for another program.
    Json,
}

/// The measurements window as it is kept between frames.
#[derive(Debug, Default)]
pub struct Panel {
    /// Whether the window is open.
    pub open: bool,
    /// The annotation whose name is being typed, with what has been typed so far.
    editing: Option<(u64, String)>,
}

impl Panel {
    /// Show the window if it is open. Gives the kind of file asked for when one of the save
    /// buttons is pressed.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        state: &AppState,
        actions: &mut Vec<Action>,
    ) -> Option<Save> {
        let mut asked = None;
        let mut open = self.open;
        egui::Window::new("Measurements")
            .open(&mut open)
            .resizable(false)
            .default_pos(pos2(OPENS_AT[0], OPENS_AT[1]))
            .default_width(WIDTH)
            .show(ctx, |ui| {
                asked = contents(ui, state, &mut self.editing, actions);
            });
        self.open = open;
        asked
    }
}

/// What the window holds: the tools, undo and redo, the list, and the buttons that save it
/// or clear it. Gives the kind of file asked for when a save button is pressed.
pub fn contents(
    ui: &mut Ui,
    state: &AppState,
    editing: &mut Option<(u64, String)>,
    actions: &mut Vec<Action>,
) -> Option<Save> {
    ui.set_width(WIDTH);
    let placement = grid(state).map(|(placement, _)| placement);
    if placement.is_none() {
        ui.label(RichText::new(NO_VOLUME).color(theme::QUIET_TEXT));
    }
    ui.horizontal(|ui| {
        for (tool, word, hover) in TOOLS {
            let button = Button::new(word).selected(state.tool == tool);
            let pressed = ui.add_enabled(placement.is_some(), button);
            if pressed.on_hover_text(hover).clicked() && state.tool != tool {
                actions.push(Action::SetTool(tool));
            }
        }
    });
    let list = &state.annotations;
    ui.horizontal(|ui| {
        let step = |ui: &mut Ui, word: &str, can: bool, hover: &str| {
            let button = ui.add_enabled(can, Button::new(word));
            button.on_hover_text(hover).clicked()
        };
        let back = "Take back the last change to this list (Ctrl+Z)";
        if step(ui, "Undo", list.can_undo(), back) {
            actions.push(Action::Undo);
        }
        if step(
            ui,
            "Redo",
            list.can_redo(),
            "Do again what was taken back (Ctrl+Y)",
        ) {
            actions.push(Action::Redo);
        }
    });
    ui.separator();
    if list.items.is_empty() {
        let none = "Nothing has been measured on this series yet.";
        ui.label(RichText::new(none).color(theme::QUIET_TEXT));
    }
    for item in &list.items {
        let said = measured(&item.shape, placement.as_ref());
        row(ui, item, (&said.value, &said.detail), editing, actions);
    }
    ui.separator();
    kept_or_cleared(ui, !list.items.is_empty(), actions)
}

/// The buttons along the bottom, greyed while nothing is listed. Gives the kind of file asked
/// for when a save button is pressed.
fn kept_or_cleared(ui: &mut Ui, any: bool, actions: &mut Vec<Action>) -> Option<Save> {
    let mut asked = None;
    ui.horizontal(|ui| {
        let table = ui.add_enabled(any, Button::new("Save as CSV…"));
        if table
            .on_hover_text("A table any spreadsheet opens")
            .clicked()
        {
            asked = Some(Save::Csv);
        }
        let json = ui.add_enabled(any, Button::new("Save as JSON…"));
        if json.on_hover_text("For another program to read").clicked() {
            asked = Some(Save::Json);
        }
        let clear = ui.add_enabled(any, Button::new("Clear all"));
        let hover = "Remove every measurement of this series (Ctrl+Z brings them back)";
        if clear.on_hover_text(hover).clicked() {
            actions.push(Action::AnnotationsClear);
        }
    });
    asked
}

/// One annotation in the list: its name, which can be typed over, what it measures, which
/// can be pressed to go to it, and a button that removes it; beneath, what qualifies it.
fn row(
    ui: &mut Ui,
    item: &Annotation,
    (value, detail): (&str, &str),
    editing: &mut Option<(u64, String)>,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        let typed = editing.as_ref().filter(|(id, _)| *id == item.id);
        let mut name = typed.map_or_else(|| item.label.clone(), |(_, typed)| typed.clone());
        let field = TextEdit::singleline(&mut name)
            .desired_width(NAME_WIDTH)
            .char_limit(MAX_LABEL)
            .id_salt(("annotation name", item.id));
        let field = ui.add(field);
        if field.changed() || field.has_focus() {
            *editing = Some((item.id, name.clone()));
        }
        if field.lost_focus() {
            // The name is taken when the person leaves the field, not at every letter.
            *editing = None;
            if name != item.label {
                let (id, label) = (item.id, name);
                actions.push(Action::AnnotationRename { id, label });
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("Remove").clicked() {
                actions.push(Action::AnnotationRemove(item.id));
            }
            // What is left between the name and that button is the value's, cut short if
            // it is longer.
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let go = ui.add(Button::new(value).frame(false).truncate());
                if go.on_hover_text("Go to it").clicked() {
                    let first = item.shape.points().into_iter().next();
                    let there = first.map(|[x, y, z]| Point::new(x, y, z));
                    actions.extend(there.map(Action::SetPosition));
                }
            });
        });
    });
    if !detail.is_empty() {
        ui.label(RichText::new(detail).small().color(theme::QUIET_TEXT));
    }
}

/// Ask where to save the measurements of the series in the scene, write them there in the
/// kind of file asked for, and say how it went. `None` when the person chose nowhere.
pub fn save(kind: Save, state: &AppState) -> Option<Notice> {
    let (title, what, ending) = match kind {
        Save::Csv => ("Save the measurements as a table", "CSV table", "csv"),
        Save::Json => ("Save the measurements as JSON", "JSON", "json"),
    };
    let path = rfd::FileDialog::new()
        .set_title(title)
        .set_file_name(format!("{MEASUREMENTS_NAME}.{ending}"))
        .add_filter(what, &[ending])
        .save_file()?;
    let placement = grid(state).map(|(placement, _)| placement);
    let rows = rows(&state.annotations.items, placement.as_ref());
    let text = match kind {
        Save::Csv => measurements_csv(&rows),
        Save::Json => measurements_json(&rows, theme::DISCLAIMER),
    };
    Some(said(&path, write(&path, text.as_bytes())))
}

#[cfg(test)]
mod tests;
