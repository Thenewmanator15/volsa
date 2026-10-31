//! The Measure tab: the tools, undo and redo, the list of what has been measured on the series
//! in the scene, and saving it. Its buttons are those of the list of actions, so their words
//! and hints are written there and nowhere else.

use egui::{Align, Button, Layout, RichText, TextEdit, Ui};
use voxa_core::Point;

use super::super::commands::{self, Request};
use super::super::scene::export::said;
use super::super::theme;
use super::{offer, row, section};
use crate::action::Action;
use crate::annotations::{Annotation, MAX_LABEL, Tool, measured};
use crate::export::write;
use crate::measurements::{MEASUREMENTS_NAME, measurements_csv, measurements_json, rows};
use crate::measuring::{
    Scale, SlicePlacement, plain_stack_note, scene_placement, why_not_measurable,
};
use crate::state::{AppState, Notice};

/// What is said while nothing has been measured on the series: in place of its list, and as
/// the reason the list cannot be saved or cleared.
pub const NOTHING_MEASURED: &str = "Nothing has been measured on this series yet.";

/// Each tool with the action that takes it in hand.
const TOOLS: [(Tool, &str); 5] = [
    (Tool::Navigate, "measure.move"),
    (Tool::Ruler, "measure.ruler"),
    (Tool::Angle, "measure.angle"),
    (Tool::Region, "measure.region"),
    (Tool::Marker, "measure.marker"),
];

/// The actions that take back the last change to the list and do it again.
const STEPS: [&str; 2] = ["measure.undo", "measure.redo"];

/// The actions along the bottom: the two ways to save the list, and clearing it.
const KEEPING: [&str; 3] = ["measure.save_csv", "measure.save_json", "measure.clear"];

/// Which kind of file the measurements are to be saved as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Save {
    /// A table any spreadsheet opens.
    Csv,
    /// JSON, for another program.
    Json,
}

/// The Measure tab as it is kept between frames.
#[derive(Debug, Default)]
pub struct MeasureTab {
    /// The annotation whose name is being typed, with what has been typed so far.
    editing: Option<(u64, String)>,
}

/// Draw the Measure tab: the tools, undo and redo, the list, and the buttons that save it or
/// clear it. Changes of state go onto `actions`; what its buttons ask of the window, such as
/// saving, is given back.
pub fn show(
    ui: &mut Ui,
    state: &AppState,
    tab: &mut MeasureTab,
    actions: &mut Vec<Action>,
) -> Vec<Request> {
    let mut asked = Vec::new();
    if let Some(why) = why_not_measurable(state) {
        ui.label(RichText::new(why).color(theme::QUIET_TEXT));
    }
    section(ui, "Tools");
    tools(ui, state, actions, &mut asked);
    row(ui, |ui| {
        for id in STEPS {
            offer(ui, id, state, None, actions, &mut asked);
        }
    });
    section(ui, "On this series");
    if let Some(note) = plain_stack_note(state) {
        ui.label(RichText::new(note).small().color(theme::QUIET_TEXT));
    }
    let list = &state.annotations;
    if list.items.is_empty() {
        ui.label(RichText::new(NOTHING_MEASURED).color(theme::QUIET_TEXT));
    }
    let at = scene_placement(state);
    let placement = at.map(|at| at.placement);
    for item in &list.items {
        let said = measured(&item.shape, placement.as_ref(), item.scale);
        let words = (said.shown(), said.detail);
        if listed(ui, item, (&words.0, &words.1), &mut tab.editing, actions) {
            actions.extend(go_to(state, at.as_ref(), item));
        }
    }
    ui.separator();
    row(ui, |ui| {
        for id in KEEPING {
            offer(ui, id, state, None, actions, &mut asked);
        }
    });
    asked
}

/// The buttons of the tools, the one in hand drawn pressed. The tool in hand is not asked
/// for again.
fn tools(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    row(ui, |ui| {
        for (tool, id) in TOOLS {
            let in_hand = state.tool == tool;
            let pressed = commands::button(ui, id, state, Some(in_hand));
            if let Some(does) = pressed.filter(|_| !in_hand) {
                asked.extend(does.run(state, actions));
            }
        }
    });
}

/// One annotation in the list: its name, which can be typed over, and a button that removes
/// it; beneath, what it measures, which can be pressed to go to it, and what qualifies that.
/// True when it was pressed to go to it.
fn listed(
    ui: &mut Ui,
    item: &Annotation,
    (value, detail): (&str, &str),
    editing: &mut Option<(u64, String)>,
    actions: &mut Vec<Action>,
) -> bool {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        // The button is at the right and the name takes what is left of the line.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("Remove").clicked() {
                actions.push(Action::AnnotationRemove(item.id));
            }
            name(ui, item, editing, actions);
        });
    });
    let go = ui.add(Button::new(value).frame(false).truncate());
    let pressed = go.on_hover_text("Go to it").clicked();
    if !detail.is_empty() {
        ui.label(RichText::new(detail).small().color(theme::QUIET_TEXT));
    }
    pressed
}

/// What going to an annotation asks for: the planes' position at its first point, for one in
/// the patient's millimetres, and the active viewport on the slice that point lies on, where
/// `at` places the slices of the series it was measured on.
fn go_to(state: &AppState, at: Option<&SlicePlacement>, item: &Annotation) -> Vec<Action> {
    let Some([x, y, z]) = item.shape.points().into_iter().next() else {
        return Vec::new();
    };
    let first = Point::new(x, y, z);
    let in_the_patient = item.scale == Scale::Patient;
    let position = in_the_patient.then_some(Action::SetPosition(first));
    let made_at = at.filter(|at| at.scale == item.scale);
    let voxel = made_at.and_then(|at| Some(at.placement.inverse()?.apply(first)));
    let slice = voxel.and_then(|voxel| slice_number(voxel.z));
    let viewport = state.active;
    let shown = slice.map(|slice| Action::SetSlice { viewport, slice });
    position.into_iter().chain(shown).collect()
}

/// The slice nearest a place along a series' slices, counting from 0; `None` before the first.
fn slice_number(along: f64) -> Option<usize> {
    let nearest = along.round();
    (0.0..=f64::from(u32::MAX))
        .contains(&nearest)
        .then(|| whole(nearest))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "checked to be a whole number between 0 and u32::MAX first"
)]
fn whole(number: f64) -> usize {
    number as usize
}

/// The name of an annotation, in a field that fills the line. It is taken when the person
/// leaves the field, not at every letter.
fn name(
    ui: &mut Ui,
    item: &Annotation,
    editing: &mut Option<(u64, String)>,
    actions: &mut Vec<Action>,
) {
    let typed = editing.as_ref().filter(|(id, _)| *id == item.id);
    let mut text = typed.map_or_else(|| item.label.clone(), |(_, typed)| typed.clone());
    let field = TextEdit::singleline(&mut text)
        .desired_width(ui.available_width())
        .char_limit(MAX_LABEL)
        .id_salt(("annotation name", item.id));
    let field = ui.add(field);
    if field.changed() || field.has_focus() {
        *editing = Some((item.id, text.clone()));
    }
    if field.lost_focus() {
        *editing = None;
        if text != item.label {
            let (id, label) = (item.id, text);
            actions.push(Action::AnnotationRename { id, label });
        }
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
    let text = written(kind, state);
    Some(said(&path, write(&path, text.as_bytes())))
}

/// The measurements of the series in the scene, as the kind of file asked for, every one of
/// them carrying [`notice`].
fn written(kind: Save, state: &AppState) -> String {
    let placement = scene_placement(state).map(|at| at.placement);
    let rows = rows(&state.annotations.items, placement.as_ref());
    let notice = notice(state);
    match kind {
        Save::Csv => measurements_csv(&rows, &notice),
        Save::Json => measurements_json(&rows, &notice),
    }
}

/// What every saved measurement of the series in the scene carries: that Voxa is not for
/// diagnostic use, and, for a plain stack, that where its slices lie is only roughly known.
fn notice(state: &AppState) -> String {
    match plain_stack_note(state) {
        Some(note) => format!("{} {note}", theme::DISCLAIMER),
        None => theme::DISCLAIMER.to_owned(),
    }
}

#[cfg(test)]
mod slice_tests;
#[cfg(test)]
mod tests;
