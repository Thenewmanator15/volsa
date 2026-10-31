//! The Labels tab: the labelled parts of the head. Who made the labels and the model's own
//! check of them come first, before any part, so that a label never reads as a finding; then
//! an overview by group; then the list of parts, to search, show, hide, fade and select.
//! Without labels the tab says what they are and the two ways to get some.

use egui::{Align, RichText, ScrollArea, Ui, style::ScrollAnimation};
use voxa_render::Overlay;

use super::{offer, row, section};
use crate::action::Action;
use crate::labels::{Labels, LabelsStatus, NamesFrom, on_this_series};
use crate::state::AppState;
use crate::ui::access;
use crate::ui::commands::Request;
use crate::ui::parts::{
    self, Asked, MADE_ELSEWHERE, PartsView, WHAT_LABELS_ARE, all_shown, labelling, title,
};
use crate::ui::theme;

mod overview;

/// Said of labels whose names came from a table beside them.
const NAMED: &str = "Names from the table beside the labels.";
/// Said of labels whose names are the model's own.
const NAMED_BY_THE_MODEL: &str = "Names from the model's own list.";
/// Said of labels that have no names.
const NOT_NAMED: &str = "No table of names: the parts are called by their numbers.";
/// Said of labels that belong to another study than the one shown.
const OTHER_STUDY: &str = "The labels open belong to another study than the one shown.";
/// Said beside the button that shows the Tools tab, where a scan is labelled.
const LABEL_IN_TOOLS: &str = "Label this scan in the Tools tab.";

/// Every sentence the tab keeps in a constant, for the check that none suggests a finding.
#[cfg(test)]
pub(super) const TEXTS: [&str; 5] = [
    NAMED,
    NAMED_BY_THE_MODEL,
    NOT_NAMED,
    OTHER_STUDY,
    LABEL_IN_TOOLS,
];

/// A tab this wide has room for each part's volume beside its name.
const WIDE: f32 = 300.0;

/// The words for a quiet line.
fn quiet(ui: &Ui, text: &str) -> RichText {
    let quiet = theme::palette(ui.ctx()).quiet_text;
    RichText::new(text).small().color(quiet)
}

/// The most of the tab's height the part that stays in place may take; if it would take more,
/// it scrolls too, so that the list keeps room.
const FIXED_MOST: f32 = 0.6;
/// The name the scroll area of the part that stays in place keeps its place under.
const FIXED_SCROLL: &str = "voxa: the Labels tab, the part that stays in place";
/// The name the scroll area of the list keeps its place under. It has its own, as the panel's
/// scroll area, which the other tabs scroll in, has one.
const LIST_SCROLL: &str = "voxa: the Labels tab, the list";

/// A part of the tab that scrolls, and takes the height that is left.
fn scrolling<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let area = ScrollArea::vertical()
        .id_salt(LIST_SCROLL)
        .auto_shrink([false, false]);
    area.show(ui, add).inner
}

/// Draw the Labels tab, in the panel's own room: it scrolls itself, so that with labels open
/// what stays in place is in sight while the list scrolls. Changes of state go onto
/// `actions`; what its buttons ask of the window, such as choosing a file of labels to open,
/// is given back.
pub fn show(
    ui: &mut Ui,
    state: &AppState,
    view: &mut PartsView,
    actions: &mut Vec<Action>,
) -> Vec<Request> {
    let mut asked = Vec::new();
    match &state.labels.status {
        LabelsStatus::Ready if on_this_series(state) => {
            list(ui, &state.labels, view, actions, &mut asked);
        }
        status => scrolling(ui, |ui| {
            without_labels(ui, state, status, actions, &mut asked);
        }),
    }
    asked
}

/// What the tab says while there are no labels to list: what they are, that they are being
/// read, why they could not be, or that those open belong to another study.
fn without_labels(
    ui: &mut Ui,
    state: &AppState,
    status: &LabelsStatus,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    match status {
        LabelsStatus::None => {
            ui.label(quiet(ui, WHAT_LABELS_ARE));
            ways_to_get_some(ui, state, actions, asked);
        }
        LabelsStatus::Reading => {
            ui.label("Reading the labels…");
        }
        LabelsStatus::Failed(why) => {
            ui.label(RichText::new(why).color(theme::palette(ui.ctx()).warning));
            ways_to_get_some(ui, state, actions, asked);
        }
        LabelsStatus::Ready => {
            ui.label(quiet(ui, OTHER_STUDY));
            if ui.button("Close").clicked() {
                actions.push(Action::CloseLabels);
            }
        }
    }
}

/// The two ways to get labels: open a file of them, or label the scan in the Tools tab.
fn ways_to_get_some(
    ui: &mut Ui,
    state: &AppState,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    offer(ui, "labels.open", state, None, actions, asked);
    row(ui, |ui| {
        ui.label(quiet(ui, LABEL_IN_TOOLS));
        offer(ui, "tab.tools", state, None, actions, asked);
    });
}

/// The labels open on the series. In place at the top: what they are and who made them, the
/// switches that act on all of them, the search box and the part selected. Scrolling beneath
/// them: the overview by group, and every group with its rows.
fn list(
    ui: &mut Ui,
    labels: &Labels,
    view: &mut PartsView,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    // In a short window the part that stays in place would leave the list almost no room, so
    // it scrolls too, with the model's check, which is the first of it, in sight to begin with.
    let fixed = ScrollArea::vertical()
        .id_salt(FIXED_SCROLL)
        .auto_shrink([false, true])
        .max_height(FIXED_MOST * ui.available_height());
    fixed.show(ui, |ui| {
        asked.extend(about(ui, labels));
        asked.extend(switches(ui, labels, actions));
        let search = egui::TextEdit::singleline(&mut view.search).hint_text("Search parts");
        let search = ui.add(search.desired_width(f32::INFINITY));
        access::call(&search, "Search parts");
        if let Some(part) = labels.selected {
            parts::chosen(ui, labels, part, actions);
        }
    });
    ui.separator();
    scrolling(ui, |ui| {
        section(ui, "Groups");
        if let Some(group) = overview::show(ui, labels, actions) {
            // A group the search leaves out cannot be scrolled to: the search is let go.
            if !parts::rows(labels, &view.search)
                .iter()
                .any(|one| one.name == group)
            {
                view.search.clear();
            }
            view.scroll_to = Some(group);
        }
        ui.separator();
        groups(ui, labels, view, actions);
    });
}

/// Every group of the list with its rows, those with the letters typed in the search box.
fn groups(ui: &mut Ui, labels: &Labels, view: &mut PartsView, actions: &mut Vec<Action>) {
    let wide = ui.available_width() >= WIDE;
    let found = parts::rows(labels, &view.search);
    if found.is_empty() {
        ui.label(
            RichText::new("No part has that in its name.")
                .color(theme::palette(ui.ctx()).quiet_text),
        );
    }
    for group in found {
        let mut all = all_shown(labels, &group);
        let name = RichText::new(title(&group)).small().strong();
        let header = ui.checkbox(&mut all, name);
        if view.scroll_to.as_deref() == Some(group.name.as_str()) {
            view.scroll_to = None;
            header.scroll_to_me_animation(Some(Align::TOP), ScrollAnimation::none());
        }
        if header
            .on_hover_text("Show or hide the whole group")
            .changed()
        {
            let (group, shown) = (group.name.clone(), all);
            actions.push(Action::GroupShow { group, shown });
        }
        for pair in &group.rows {
            parts::row(ui, labels, pair, wide, actions);
        }
    }
}

/// What the labels are: what they are of, who made them, where their names came from, and the
/// labelling model's own check of them; and, beneath that, the button that saves the list as
/// a table, and for labels that Voxa made, the one that saves them as a file. Gives what was
/// asked of the window.
fn about(ui: &mut Ui, labels: &Labels) -> Vec<Request> {
    if !labels.title.is_empty() {
        ui.label(RichText::new(&labels.title).strong());
    }
    // Labels that Voxa made say so, and what they cannot show, with the model's check first.
    match &labels.made {
        Some(made) => labelling::made_by(ui, labels.verdict.as_ref(), made),
        None => {
            ui.label(quiet(ui, MADE_ELSEWHERE));
        }
    }
    ui.label(quiet(
        ui,
        match labels.names_from {
            NamesFrom::Table => NAMED,
            NamesFrom::Model => NAMED_BY_THE_MODEL,
            NamesFrom::Nothing => NOT_NAMED,
        },
    ));
    if let Some(verdict) = labels.verdict.as_ref().filter(|_| labels.made.is_none()) {
        labelling::verdict_line(ui, verdict);
    }
    let save = ui.small_button("Save as CSV…");
    let hint = "Save every part, with its volume, as a table for a spreadsheet";
    let csv = save.on_hover_text(hint).clicked().then_some(Asked::SaveCsv);
    let file = labelling::save_as(ui, labels).map(Asked::Labelling);
    csv.into_iter().chain(file).map(Request::Choose).collect()
}

/// The switches that act on all the labels: show every part, close the labels, choose
/// another table of names, and how the labels are drawn over the plane views.
fn switches(ui: &mut Ui, labels: &Labels, actions: &mut Vec<Action>) -> Vec<Request> {
    let mut asked = Vec::new();
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
            asked.push(Request::Choose(Asked::OpenNames));
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new("On planes")
                .small()
                .color(theme::palette(ui.ctx()).quiet_text),
        );
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

#[cfg(test)]
mod tests;
