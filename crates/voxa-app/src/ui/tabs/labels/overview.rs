//! The overview of the Labels tab: one row for each group of parts, with the group's colour,
//! its name, how many parts it holds, and a switch that shows or hides all of them. A click
//! on a group's name takes the list to it.

use egui::{Align, Layout, RichText, Ui};

use crate::action::Action;
use crate::labels::Labels;
use crate::ui::parts::{self, Group, all_shown, parts_in, title};
use crate::ui::theme;

/// How many parts a group holds, in words.
fn count_in_words(count: usize) -> String {
    match count {
        1 => "1 part".to_owned(),
        _ => format!("{count} parts"),
    }
}

/// Name `response` for a screen reader as `name`, whatever its button says.
fn named(ui: &Ui, response: &egui::Response, name: &str) {
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(name));
}

/// Draw the row of one group. Gives the group's name if its name was clicked.
fn row(ui: &mut Ui, labels: &Labels, group: &Group, actions: &mut Vec<Action>) -> Option<String> {
    let name = title(group);
    let all = all_shown(labels, group);
    let mut clicked = None;
    ui.horizontal(|ui| {
        if let Some(colour) = parts::colour_of(labels, group) {
            parts::swatch(ui, colour);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let word = if all { "Hide" } else { "Show" };
            let toggle = ui.small_button(word);
            named(ui, &toggle, &format!("{word} {name}"));
            if toggle
                .on_hover_text("Show or hide the whole group")
                .clicked()
            {
                let group = group.name.clone();
                actions.push(Action::GroupShow { group, shown: !all });
            }
            let count = count_in_words(parts_in(group));
            ui.label(RichText::new(count).small().color(theme::QUIET_TEXT));
            // What is left of the line is the name's, cut short if it is longer.
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let button = egui::Button::new(RichText::new(name).small());
                let go = ui.add(button.frame(false).truncate());
                named(ui, &go, &format!("Go to {name}"));
                if go.on_hover_text("Go to this group in the list").clicked() {
                    clicked = Some(group.name.clone());
                }
            });
        });
    });
    clicked
}

/// Draw the overview: a row for each group of the labels, in the table's order. Gives the name
/// of the group whose name was clicked, for the list to be taken to it.
pub fn show(ui: &mut Ui, labels: &Labels, actions: &mut Vec<Action>) -> Option<String> {
    let mut clicked = None;
    for group in parts::rows(labels, "") {
        clicked = row(ui, labels, &group, actions).or(clicked);
    }
    clicked
}
