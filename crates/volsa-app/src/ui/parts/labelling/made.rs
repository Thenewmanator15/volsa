//! How labels that Volsa made are described: what the list says of them, and what the scene and
//! the plane views say of the solids and outlines drawn from them.

use egui::{RichText, Ui};
use volsa_labels::{Made, Verdict, added_in_words, verdict_in_words};

use super::super::super::theme;
use super::quiet;
use crate::action::Action;
use crate::state::AppState;
use crate::ui::commands::{self, Request};

/// What is said after how labels were made.
const NOT_FOR_DIAGNOSIS: &str = "Not for diagnostic use.";

/// How labels were made, with whatever of it is known: Volsa made them, with a model, on a
/// device, on a day. A part that was not written down is left out.
fn how_made(made: &Made) -> String {
    let mut how = "made by Volsa".to_owned();
    let parts = [
        (" with ", &made.model),
        (", on ", &made.device),
        (", ", &made.when),
    ];
    for (joint, part) in parts {
        if !part.is_empty() {
            how.push_str(joint);
            how.push_str(part);
        }
    }
    how
}

/// Where and when a plugin the person added ran, with whatever of it is known, as a sentence;
/// empty when neither was written down.
fn where_it_ran(made: &Made) -> String {
    let known: Vec<&str> = [made.device.as_str(), made.when.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();
    if known.is_empty() {
        return String::new();
    }
    format!(" Run on {}.", known.join(", "))
}

/// What the list says of labels that Volsa made, in place of "made by another program". Labels
/// that a plugin the person added made say that first, as the banner every result of one
/// carries, and then where and when it ran.
pub fn made_in_words(made: &Made) -> String {
    match &made.added {
        Some(added) => format!("{}{}", added_in_words(added), where_it_ran(made)),
        None => format!("Labels {}. {NOT_FOR_DIAGNOSIS}", how_made(made)),
    }
}

/// What the scene says of the solids drawn from labels that Volsa made, or that a plugin the
/// person added made.
pub fn solids_in_words(made: &Made) -> String {
    match &made.added {
        Some(added) => format!(
            "Solids are labels added by you, not part of Volsa: {} {}. {NOT_FOR_DIAGNOSIS}",
            added.name.trim(),
            added.version.trim()
        ),
        None => format!("Solids are labels {}. {NOT_FOR_DIAGNOSIS}", how_made(made)),
    }
}

/// The labelling model's own check, in plain words with none of its scores on the screen: a lay
/// reader could take a score for a grade of their own anatomy. The scores are the model's own
/// figures, and are in the hover text. A passed check says whose opinion it is; one that did
/// not pass says for what, and that every part is to be taken with caution.
pub fn verdict_in_plain_words(verdict: &Verdict) -> String {
    if verdict.passed && verdict.low.is_empty() {
        return concat!(
            "The labelling model's own check of its work passed for every part. That is the ",
            "model's opinion of its own work, not a check against anything else."
        )
        .to_owned();
    }
    let names: Vec<&str> = verdict.low.iter().map(|(name, _)| name.as_str()).collect();
    let which = if names.is_empty() {
        String::new()
    } else {
        format!(" for {}", names.join(", "))
    };
    format!(
        "The labelling model's own check of its work did not pass{which}. Take every part here \
         with caution: labels look as neat on a poor scan as on a good one."
    )
}

/// The labelling model's own check, in plain words, in the colour of how it went, and its
/// figures when the pointer rests on it.
pub fn verdict_line(ui: &mut Ui, verdict: &Verdict) {
    let colour = if verdict.passed {
        theme::palette(ui.ctx()).quiet_text
    } else {
        theme::palette(ui.ctx()).warning
    };
    let words = RichText::new(verdict_in_plain_words(verdict)).small();
    let figures = format!("The model's own figures. {}", verdict_in_words(verdict));
    ui.label(words.color(colour)).on_hover_text(figures);
}

/// What the list says of labels that Volsa made: the model's own check of them first, so that a
/// failed check is the first thing said of the result; then who made them, with what, where
/// and when; then each note a reader should know, each on its own line.
pub fn made_by(ui: &mut Ui, verdict: Option<&Verdict>, made: &Made) {
    if let Some(verdict) = verdict {
        verdict_line(ui, verdict);
    }
    ui.label(quiet(ui, &made_in_words(made)));
    for note in &made.notes {
        ui.label(quiet(ui, note));
    }
}

/// The button that saves labels that Volsa made as a file, with a table of their names beside
/// it. Labels from a file have no such button: the file is already there.
pub fn save_as(
    ui: &mut Ui,
    state: &AppState,
    (actions, asked): (&mut Vec<Action>, &mut Vec<Request>),
) {
    if state.labels.made.is_some() {
        commands::offer_small(ui, "labels.save", state, None, (actions, asked));
    }
}
