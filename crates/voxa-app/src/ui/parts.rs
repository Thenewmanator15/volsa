//! The Parts list: the labelled parts of the head as a label file gives them, by group, to
//! search, show, hide, fade and select. It says where the names came from and how far each
//! can be trusted: Voxa shows these labels and did not make them.

use egui::{Align, Layout, Rect, RichText, ScrollArea, Sense, Ui, UiBuilder, vec2};
use voxa_labels::{Pair, Side, Trust, pairs, trust_in_words, verdict_in_words, volume_in_words};
use voxa_render::Overlay;

use super::theme;
use crate::action::Action;
use crate::labels::{FAINTEST, Labels, LabelsStatus, NamesFrom, on_this_series};
use crate::parts_table::sides_compared;
use crate::planes::grid;
use crate::state::AppState;

/// What labels are, said where there are none.
pub const WHAT_LABELS_ARE: &str = "Labels are parts of the head that another program has outlined on \
                                   this scan: a NIfTI file (.nii or .nii.gz), with a table of names \
                                   beside it if it has one. Voxa shows them; it does not make them.";
/// Said above every list of parts.
pub const MADE_ELSEWHERE: &str = "Made by another program. Voxa shows these labels as given.";
const NAMED: &str = "Names from the table beside the labels.";
const NOT_NAMED: &str = "No table of names: the parts are called by their numbers.";
const OTHER_STUDY: &str = "The labels open belong to another study than the one shown.";
const NO_VOLUME: &str = "Labels are put on the 3D view of a series, and this one has none yet.";
/// A column this wide has room for each part's volume beside its name.
const WIDE: f32 = 300.0;
/// The side of a part's swatch of colour, in points.
const SWATCH: f32 = 10.0;

/// What the person asked for that needs a file chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// To open a label file.
    OpenLabels,
    /// To choose another table of names for the labels open.
    OpenNames,
    /// To save the list of parts as a table.
    SaveCsv,
}

/// What the list keeps between frames.
#[derive(Debug, Default)]
pub struct PartsView {
    /// What is typed in the search box.
    search: String,
}

/// One group of the list, with its names: a name with a left and a right is one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The group's name; empty when the table gave none.
    pub name: String,
    /// Its rows, in the table's order.
    pub rows: Vec<Pair>,
}

/// The groups and rows the list shows for what is typed in the search box: every row whose
/// name or group has those letters in it, whatever their case, under its group, in the
/// table's order. Groups left with no rows are left out.
pub fn rows(labels: &Labels, search: &str) -> Vec<Group> {
    let wanted = search.trim().to_lowercase();
    let found = |pair: &Pair| {
        pair.name.to_lowercase().contains(&wanted) || pair.group.to_lowercase().contains(&wanted)
    };
    let mut groups: Vec<Group> = Vec::new();
    for pair in pairs(&labels.parts).into_iter().filter(found) {
        match groups.iter_mut().find(|group| group.name == pair.group) {
            Some(group) => group.rows.push(pair),
            None => groups.push(Group {
                name: pair.group.clone(),
                rows: vec![pair],
            }),
        }
    }
    groups
}

/// Draw the list in `area`. Gives what was asked for that needs a file chosen.
pub fn show(
    ui: &mut Ui,
    area: Rect,
    state: &AppState,
    view: &mut PartsView,
    actions: &mut Vec<Action>,
) -> Option<Asked> {
    ui.painter().rect_filled(area, 0, theme::PANEL);
    let mut inside = ui.new_child(UiBuilder::new().max_rect(area.shrink(6.0)));
    inside.set_clip_rect(area);
    contents(&mut inside, state, view, actions)
}

/// What the Parts tab holds: the list, or why there is none.
pub fn contents(
    ui: &mut Ui,
    state: &AppState,
    view: &mut PartsView,
    actions: &mut Vec<Action>,
) -> Option<Asked> {
    let quiet = |text: &str| RichText::new(text).small().color(theme::QUIET_TEXT);
    let open = |ui: &mut Ui| {
        let can = grid(state).is_some();
        let button = ui.add_enabled(can, egui::Button::new("Open labels…"));
        let button = button.on_disabled_hover_text(NO_VOLUME);
        button.clicked().then_some(Asked::OpenLabels)
    };
    match &state.labels.status {
        LabelsStatus::None => {
            ui.label(quiet(WHAT_LABELS_ARE));
            open(ui)
        }
        LabelsStatus::Reading => {
            ui.label("Reading the labels…");
            None
        }
        LabelsStatus::Failed(why) => {
            ui.label(RichText::new(why).color(theme::WARNING));
            open(ui)
        }
        LabelsStatus::Ready if !on_this_series(state) => {
            ui.label(quiet(OTHER_STUDY));
            if ui.button("Close").clicked() {
                actions.push(Action::CloseLabels);
            }
            None
        }
        LabelsStatus::Ready => list(ui, &state.labels, view, actions),
    }
}

/// The list itself: the switches that act on all the labels, the search box and the part
/// selected, which stay in place; then, scrolling beneath them, what the labels are and
/// every group with its rows.
fn list(
    ui: &mut Ui,
    labels: &Labels,
    view: &mut PartsView,
    actions: &mut Vec<Action>,
) -> Option<Asked> {
    let mut asked = switches(ui, labels, actions);
    let search = egui::TextEdit::singleline(&mut view.search).hint_text("Search parts");
    ui.add(search.desired_width(f32::INFINITY));
    if let Some(part) = labels.selected {
        chosen(ui, labels, part, actions);
    }
    ui.separator();
    let wide = ui.available_width() >= WIDE;
    let tall = ScrollArea::vertical().auto_shrink([false, false]);
    tall.show(ui, |ui| {
        asked = asked.or(about(ui, labels));
        let groups = rows(labels, &view.search);
        if groups.is_empty() {
            ui.label(RichText::new("No part has that in its name.").color(theme::QUIET_TEXT));
        }
        for group in groups {
            let every = group.rows.iter().flat_map(sides);
            let mut all = every.clone().all(|(_, part)| labels.is_shown(part));
            let name = if group.name.is_empty() {
                "Parts"
            } else {
                &group.name
            };
            let header = ui.checkbox(&mut all, RichText::new(name).small().strong());
            if header
                .on_hover_text("Show or hide the whole group")
                .changed()
            {
                let (group, shown) = (group.name.clone(), all);
                actions.push(Action::GroupShow { group, shown });
            }
            for pair in &group.rows {
                row(ui, labels, pair, wide, actions);
            }
        }
    });
    asked
}

/// What the labels are: what they are of, that another program made them, where their
/// names came from, and the labelling model's own check of them; and, beneath that, the
/// button that saves the list as a table. Gives that it was asked for.
fn about(ui: &mut Ui, labels: &Labels) -> Option<Asked> {
    let quiet = |text: &str| RichText::new(text).small().color(theme::QUIET_TEXT);
    if !labels.title.is_empty() {
        ui.label(RichText::new(&labels.title).strong());
    }
    ui.label(quiet(MADE_ELSEWHERE));
    let named = labels.names_from == NamesFrom::Table;
    ui.label(quiet(if named { NAMED } else { NOT_NAMED }));
    if let Some(verdict) = &labels.verdict {
        let colour = if verdict.passed {
            theme::QUIET_TEXT
        } else {
            theme::WARNING
        };
        ui.label(
            RichText::new(verdict_in_words(verdict))
                .small()
                .color(colour),
        );
    }
    let save = ui.small_button("Save as CSV…");
    let hint = "Save every part, with its volume, as a table for a spreadsheet";
    save.on_hover_text(hint).clicked().then_some(Asked::SaveCsv)
}

/// The switches that act on all the labels: show every part, close the labels, choose
/// another table of names, and how the labels are drawn over the plane views.
fn switches(ui: &mut Ui, labels: &Labels, actions: &mut Vec<Action>) -> Option<Asked> {
    let mut asked = None;
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("Show all").clicked() {
            actions.push(Action::PartsShowAll);
        }
        if ui
            .small_button("Close")
            .on_hover_text("Close these labels")
            .clicked()
        {
            actions.push(Action::CloseLabels);
        }
        let names = ui.small_button("Names…");
        if names
            .on_hover_text("Choose a table of names for these labels")
            .clicked()
        {
            asked = Some(Asked::OpenNames);
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("On planes").small().color(theme::QUIET_TEXT));
        let ways = [
            (Overlay::Off, "Off", "No labels over the plane views"),
            (Overlay::Outline, "Outline", "A line round each part"),
            (
                Overlay::Filled,
                "Filled",
                "The line, and a tint of the part's colour inside it",
            ),
        ];
        for (overlay, word, hover) in ways {
            let chosen = labels.overlay == overlay;
            let way = ui.selectable_label(chosen, RichText::new(word).small());
            if way.on_hover_text(hover).clicked() && !chosen {
                actions.push(Action::SetOverlay(overlay));
            }
        }
    });
    asked
}

/// The sides a row has, each with the letter on its switch and its part.
fn sides(pair: &Pair) -> impl Iterator<Item = (&'static str, usize)> + Clone {
    let marked = [("L", pair.left), ("R", pair.right), ("", pair.midline)];
    marked
        .into_iter()
        .filter_map(|(mark, part)| Some((mark, part?)))
}

/// What is said of a row when it is pointed at: the volume of each side, and how far the
/// name can be trusted.
fn details(labels: &Labels, pair: &Pair) -> String {
    let mut said = Vec::new();
    for (mark, part) in sides(pair) {
        let Some(part) = labels.parts.get(part) else {
            continue;
        };
        let side = match mark {
            "L" => "left ",
            "R" => "right ",
            _ => "",
        };
        said.push(format!("{side}{}", volume_in_words(part.ml)));
    }
    let volumes = said.join(", ");
    let first = sides(pair)
        .next()
        .and_then(|(_, part)| labels.parts.get(part));
    match first.map(|part| &part.trust) {
        Some(Trust::Approximate(how)) if !how.is_empty() => {
            format!("{volumes}\nApproximate. {how}")
        }
        Some(trust) => format!("{volumes}\nHow it was made: {}", trust_in_words(trust)),
        None => volumes,
    }
}

/// One row: a swatch of the part's colour, its name, which selects it, and a switch for
/// each side it has; with room, the volume of each side too.
fn row(ui: &mut Ui, labels: &Labels, pair: &Pair, wide: bool, actions: &mut Vec<Action>) {
    let parts: Vec<(&str, usize)> = sides(pair).collect();
    let Some(first) = parts.first().and_then(|(_, part)| labels.parts.get(*part)) else {
        return;
    };
    ui.horizontal(|ui| {
        let (swatch, _) = ui.allocate_exact_size(vec2(SWATCH, SWATCH), Sense::hover());
        let [red, green, blue] = first.colour;
        ui.painter()
            .rect_filled(swatch, 2, egui::Color32::from_rgb(red, green, blue));
        let selected = parts.iter().any(|(_, part)| labels.selected == Some(*part));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for (mark, part) in parts.iter().rev() {
                let mut shown = labels.is_shown(*part);
                if ui
                    .checkbox(&mut shown, RichText::new(*mark).small())
                    .changed()
                {
                    actions.push(Action::PartShow { part: *part, shown });
                }
            }
            if first.trust != Trust::Model {
                let word = trust_in_words(&first.trust);
                ui.label(RichText::new(word).small().color(theme::QUIET_TEXT));
            }
            if wide {
                let volumes = parts.iter().filter_map(|(_, part)| labels.parts.get(*part));
                let volumes: Vec<String> = volumes.map(|part| volume_in_words(part.ml)).collect();
                ui.label(
                    RichText::new(volumes.join(" | "))
                        .small()
                        .color(theme::QUIET_TEXT),
                );
            }
            // What is left of the line is the name's, cut short if it is longer.
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let name = egui::Button::new(RichText::new(&pair.name).small());
                let name = ui.add(name.selected(selected).frame(selected).truncate());
                if name.on_hover_text(details(labels, pair)).clicked() {
                    let shown = parts.iter().map(|(_, part)| *part);
                    let pick = shown.clone().find(|part| labels.is_shown(*part));
                    actions.push(Action::PartSelect(pick.filter(|_| !selected)));
                }
            });
        });
    });
}

/// The part selected: what it is, how solid it is drawn, showing it alone, and its two
/// sides set against each other if it has two.
fn chosen(ui: &mut Ui, labels: &Labels, part: usize, actions: &mut Vec<Action>) {
    let (Some(named), Some(how)) = (labels.parts.get(part), labels.shown.get(part)) else {
        return;
    };
    ui.separator();
    let side = match named.side {
        Side::Left => ", left",
        Side::Right => ", right",
        Side::Midline => "",
    };
    let volume = volume_in_words(named.ml);
    ui.label(RichText::new(format!("{}{side}: {volume}", named.name)).strong());
    ui.horizontal_wrapped(|ui| {
        let mut opacity = how.opacity;
        let solid = egui::Slider::new(&mut opacity, FAINTEST..=1.0).show_value(false);
        if ui
            .add(solid)
            .on_hover_text("How solid it is drawn")
            .changed()
        {
            actions.push(Action::PartOpacity { part, opacity });
        }
        if ui.small_button("Show only this").clicked() {
            actions.push(Action::PartIsolate(part));
        }
    });
    if let Some(against) = sides_compared(labels, part) {
        ui.label(RichText::new(against).small().color(theme::QUIET_TEXT));
    }
}

#[cfg(test)]
mod tests;
