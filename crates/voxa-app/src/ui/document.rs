//! A document of the export, shown in the middle of the window in place of the pictures: what
//! kind of document it is and its title, and then by kind. A structured report is shown as
//! text, headed as not the reference, with its lines indented by how deep they lie and the
//! text selectable. A PDF is not shown at all: Voxa has no PDF reader, and starts no other
//! program for one; it offers a copy of the PDF to save where the person chooses. Any other
//! file Voxa lists is named, and said to be one Voxa cannot show. Each has the tags of its
//! file one button away.
//!
//! What a report says is shown as the report wrote it, and the original report is the
//! reference: the lines are the start of it, in its order, and when part was left out the
//! window says so (see S-UI-01 in `SECURITY.md`).

use std::path::PathBuf;

use egui::{Align, Button, Frame, Label, Layout, Margin, Rect, RichText, Ui};
use egui::{UiBuilder, pos2, vec2};
use voxa_dicom::{DocumentKind, IndexedDocument};

use super::commands::{self, Request, TagsOf};
use super::rail::{date_of, title_of};
use super::theme;
use crate::action::Action;
use crate::documents::{Mask, kind_words};
use crate::state::{AppState, OpenDocument};

mod lines;

pub use lines::Kept;
use lines::Shown;

/// The line above a report, which says what is shown is not the report itself.
pub const HEADING: &str = "Shown as text. The original report is the reference.";
/// The line under it that says what the report leaves out while the patient's details are
/// hidden. It is shown only when something was in fact replaced, in the report or in the title
/// above it.
pub const LEFT_OUT: &str = "While patient details are hidden, the patient's name and identifier are shown as [hidden], and a name the report gives as an item is shown as Name hidden. Turn on Show patient details to see the report as written.";
/// The line that says part of a report is not shown.
pub const CUT_SHORT: &str = "Part of this report is too long or too deeply nested to show here.";
/// What a PDF stored in DICOM is said to be.
pub const IS_A_PDF: &str = "This report is a PDF. Voxa does not show PDFs.";
/// What is said of a file that is listed and that Voxa cannot show.
pub const CANNOT_SHOW: &str = "Voxa cannot show this kind of file.";
/// What is said while a report is being read.
pub const READING: &str = "Reading the report…";
/// The button that closes the document.
pub const CLOSE: &str = "Close";
/// The name offered when a PDF is saved. It says nothing of the patient or the scan.
pub const SUGGESTED_PDF: &str = "voxa-report.pdf";
/// How far each section a line lies in moves it to the right, in points.
pub const INDENT: f32 = 18.0;
/// The room around the document, in points.
const MARGIN: i8 = 14;

/// Ask the person where to save a copy of a PDF, in the system's own dialog; `None` when they
/// chose nowhere. Nothing is written until they have chosen.
pub fn choose_pdf() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Save a copy of the PDF")
        .set_file_name(SUGGESTED_PDF)
        .add_filter("PDF", &["pdf"])
        .save_file()
}

/// Draw the document `open` in `area`; `kept` is what is kept of the lines of a report from one
/// frame to the next. A change of state goes onto `actions`; what is asked of the window (the
/// tags of the file, a place to save a copy of a PDF) is given back.
pub fn show(
    ui: &mut Ui,
    area: Rect,
    state: &AppState,
    open: &OpenDocument,
    kept: &mut Kept,
    actions: &mut Vec<Action>,
) -> Vec<Request> {
    let mut asked = Vec::new();
    ui.painter()
        .rect_filled(area, theme::ROUNDING, theme::palette(ui.ctx()).panel);
    let held = state.index.as_ref().map(|index| &index.documents);
    let document = held.and_then(|all| all.iter().find(|one| one.entry == open.entry));
    let inside = Margin::same(MARGIN);
    let frame = Frame::NONE.inner_margin(inside);
    ui.scope_builder(UiBuilder::new().max_rect(area), |ui| {
        frame.show(ui, |ui| {
            ui.set_min_size(area.size() - vec2(f32::from(2 * MARGIN), f32::from(2 * MARGIN)));
            let Some(document) = document else {
                return;
            };
            header(ui, document, state, actions, &mut asked);
            ui.separator();
            match &document.kind {
                DocumentKind::Report => report(ui, open, (document, state), kept),
                DocumentKind::Pdf => asked.extend(pdf(ui, state, actions)),
                DocumentKind::Other(_) => {
                    ui.label(CANNOT_SHOW);
                }
            }
        });
    });
    asked
}

/// The top of the document: what kind it is, its title and date, and the buttons that show the
/// tags of its file and close it. The buttons are laid out first, at the right, and the words
/// have the room that is left, cut short with an ellipsis where it runs out: a long title never
/// pushes the buttons out of the area or under it.
fn header(
    ui: &mut Ui,
    document: &IndexedDocument,
    state: &AppState,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    let full = ui.available_rect_before_wrap();
    let at_the_right = UiBuilder::new()
        .max_rect(full)
        .layout(Layout::right_to_left(Align::Min));
    let mut buttons = ui.new_child(at_the_right);
    let close = commands::button_made(&mut buttons, "document.close", state, CLOSE, |command| {
        Button::new(command.label)
    });
    if let Some((_, Some(does))) = close {
        asked.extend(does.run(state, actions));
    }
    let tags = commands::button_made(&mut buttons, "tags.show", state, "", |command| {
        Button::new(command.name)
    });
    if let Some((_, Some(_))) = tags {
        asked.push(Request::Tags(TagsOf::Entry(document.entry.clone())));
    }
    let used = buttons.min_rect();
    let edge = (used.left() - ui.spacing().item_spacing.x).max(full.left());
    let left = Rect::from_min_max(full.min, pos2(edge, full.bottom()));
    let mut words = ui.new_child(UiBuilder::new().max_rect(left));
    let kind = RichText::new(kind_words(&document.kind)).heading().strong();
    words.add(Label::new(kind).truncate());
    let mask = Mask::for_document(state, document);
    let title = format!("{} · {}", title_of(document, &mask), date_of(document));
    let quiet = RichText::new(title)
        .small()
        .color(theme::palette(ui.ctx()).quiet_text);
    words.add(Label::new(quiet).truncate());
    ui.advance_cursor_after_rect(used | words.min_rect());
}

/// A structured report: while it is being read, why it could not be, or the lines of it.
fn report(ui: &mut Ui, open: &OpenDocument, of: (&IndexedDocument, &AppState), kept: &mut Kept) {
    match &open.report {
        None => {
            ui.label(RichText::new(READING).color(theme::palette(ui.ctx()).quiet_text));
        }
        Some(Err(why)) => {
            ui.label(RichText::new(why).color(theme::palette(ui.ctx()).warning));
        }
        Some(Ok(report)) => {
            ui.label(RichText::new(HEADING).strong());
            let (document, state) = of;
            let shown = Shown {
                report,
                document,
                state,
            };
            if kept.take_in(&shown) {
                let quiet = theme::palette(ui.ctx()).quiet_text;
                ui.label(RichText::new(LEFT_OUT).small().color(quiet));
            }
            if report.cut_short {
                ui.label(RichText::new(CUT_SHORT).color(theme::palette(ui.ctx()).warning));
            }
            ui.add_space(6.0);
            lines::show(ui, &shown, kept);
        }
    }
}

/// A PDF: it is said to be one, and a copy of it can be saved, which asks the window for a
/// place to save it.
fn pdf(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> Option<Request> {
    ui.label(IS_A_PDF);
    let save = commands::button(ui, "document.save_pdf", state, None);
    save.and_then(|does| does.run(state, actions))
}

#[cfg(test)]
mod header_tests;
#[cfg(test)]
mod private_tests;
#[cfg(test)]
mod tests;
