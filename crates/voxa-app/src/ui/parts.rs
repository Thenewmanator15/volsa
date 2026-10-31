//! What the Labels tab lists: the labelled parts of the head as a label file gives them, by
//! group, to search, show, hide, fade and select. The rows, the part selected and the words
//! that say what labels are, who made them and what a part's volume is are here; the tab that
//! draws them with the overview by group, and the strip that labels a scan, are in
//! [`super::tabs`] and [`labelling`].

use egui::{Align, Layout, RichText, Sense, Ui, vec2};
use voxa_labels::{Pair, Side, Trust, pairs, trust_in_words, volume_in_words};

use super::theme;
use crate::action::Action;
use crate::labels::{FAINTEST, Labels};
use crate::parts_table::sides_compared;

pub mod labelling;

/// What labels are, said where there are none.
pub const WHAT_LABELS_ARE: &str = "Labels are parts of the head outlined on this scan: by Voxa, \
                                   when it labels the scan, or by another program, as a NIfTI \
                                   file (.nii or .nii.gz) with a table of names beside it if it \
                                   has one.";
/// Said above every list of parts.
pub const MADE_ELSEWHERE: &str = "Made by another program. Voxa shows these labels as given.";
/// Said when labels are put on the 3D view of a series that has none.
pub const NO_VOLUME: &str = "Labels are put on the 3D view of a series, and this one has none yet.";
/// The side of a part's swatch of colour, in points.
const SWATCH: f32 = 10.0;
/// What the slider that fades the part selected is for.
const SOLID_SAYS: &str = "How solid it is drawn";

/// What the person asked for that needs a file chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    /// To open a label file.
    OpenLabels,
    /// To choose another table of names for the labels open.
    OpenNames,
    /// To save the list of parts as a table.
    SaveCsv,
    /// Something the labelling strip needs chosen: a folder, or where to save the labels.
    Labelling(labelling::Wants),
}

/// What the list keeps between frames.
#[derive(Debug, Default)]
pub struct PartsView {
    /// What is typed in the search box.
    pub search: String,
    /// The group the list is to scroll to, when the overview has been clicked.
    pub scroll_to: Option<String>,
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

/// What a group is called where it is listed: its name, or "Parts" when the table gave none.
pub fn title(group: &Group) -> &str {
    if group.name.is_empty() {
        "Parts"
    } else {
        &group.name
    }
}

/// How many parts a group holds: a name with a left and a right is two.
pub fn parts_in(group: &Group) -> usize {
    group.rows.iter().flat_map(sides).count()
}

/// Whether every part of a group is shown.
pub fn all_shown(labels: &Labels, group: &Group) -> bool {
    let mut parts = group.rows.iter().flat_map(sides);
    parts.all(|(_, part)| labels.is_shown(part))
}

/// The colour of a group's first part, if it has one.
pub fn colour_of(labels: &Labels, group: &Group) -> Option<[u8; 3]> {
    let (_, first) = group.rows.iter().flat_map(sides).next()?;
    labels.parts.get(first).map(|part| part.colour)
}

/// A swatch of a part's colour, `SWATCH` points across.
pub fn swatch(ui: &mut Ui, [red, green, blue]: [u8; 3]) {
    let (swatch, _) = ui.allocate_exact_size(vec2(SWATCH, SWATCH), Sense::hover());
    let colour = egui::Color32::from_rgb(red, green, blue);
    ui.painter().rect_filled(swatch, 2, colour);
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
pub fn row(ui: &mut Ui, labels: &Labels, pair: &Pair, wide: bool, actions: &mut Vec<Action>) {
    let parts: Vec<(&str, usize)> = sides(pair).collect();
    let Some(first) = parts.first().and_then(|(_, part)| labels.parts.get(*part)) else {
        return;
    };
    ui.horizontal(|ui| {
        swatch(ui, first.colour);
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
                ui.label(
                    RichText::new(word)
                        .small()
                        .color(theme::palette(ui.ctx()).quiet_text),
                );
            }
            if wide {
                let volumes = parts.iter().filter_map(|(_, part)| labels.parts.get(*part));
                let volumes: Vec<String> = volumes.map(|part| volume_in_words(part.ml)).collect();
                ui.label(
                    RichText::new(volumes.join(" | "))
                        .small()
                        .color(theme::palette(ui.ctx()).quiet_text),
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
pub fn chosen(ui: &mut Ui, labels: &Labels, part: usize, actions: &mut Vec<Action>) {
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
        let solid = ui.add(solid);
        // A screen reader says what the slider is for, as the pointer resting on it does.
        ui.ctx()
            .accesskit_node_builder(solid.id, |node| node.set_label(SOLID_SAYS));
        if solid.on_hover_text(SOLID_SAYS).changed() {
            actions.push(Action::PartOpacity { part, opacity });
        }
        if ui.small_button("Show only this").clicked() {
            actions.push(Action::PartIsolate(part));
        }
    });
    if let Some(against) = sides_compared(labels, part) {
        ui.label(
            RichText::new(against)
                .small()
                .color(theme::palette(ui.ctx()).quiet_text),
        );
    }
}

#[cfg(test)]
mod tests;
