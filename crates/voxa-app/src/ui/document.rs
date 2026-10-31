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

use egui::{Align, Button, Frame, Label, Layout, Margin, Rect, RichText, ScrollArea, Ui};
use egui::{UiBuilder, vec2};
use voxa_dicom::{DocumentKind, IndexedDocument, Report};

use super::commands::{self, Request, TagsOf};
use super::rail::{date_of, title_of};
use super::theme;
use crate::action::Action;
use crate::documents::kind_words;
use crate::naming::printable;
use crate::state::{AppState, OpenDocument};

/// The line above a report, which says what is shown is not the report itself.
pub const HEADING: &str = "Shown as text. The original report is the reference.";
/// The line that says part of a report is not shown.
pub const CUT_SHORT: &str = "Part of this report is too long or too deeply nested to show here.";
/// What a PDF stored in DICOM is said to be.
pub const IS_A_PDF: &str = "This report is a PDF. Voxa does not show PDFs.";
/// The button that saves a copy of a PDF.
pub const SAVE_PDF: &str = "Save a copy as PDF…";
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

/// Draw the document `open` in `area`. A change of state goes onto `actions`; what is asked of
/// the window (the tags of the file, a place to save a copy of a PDF) is given back.
pub fn show(
    ui: &mut Ui,
    area: Rect,
    state: &AppState,
    open: &OpenDocument,
    actions: &mut Vec<Action>,
) -> Vec<Request> {
    let mut asked = Vec::new();
    ui.painter()
        .rect_filled(area, theme::ROUNDING, theme::PANEL);
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
                DocumentKind::Report => report(ui, open),
                DocumentKind::Pdf => pdf(ui, &open.entry, &mut asked),
                DocumentKind::Other(_) => {
                    ui.label(CANNOT_SHOW);
                }
            }
        });
    });
    asked
}

/// The top of the document: what kind it is, its title and date, and the buttons that show the
/// tags of its file and close it.
fn header(
    ui: &mut Ui,
    document: &IndexedDocument,
    state: &AppState,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(kind_words(&document.kind)).heading().strong());
            let words = format!("{} · {}", title_of(document), date_of(document));
            let quiet = RichText::new(words).small().color(theme::QUIET_TEXT);
            ui.add(Label::new(quiet).truncate());
        });
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if ui.button(CLOSE).clicked() {
                actions.push(Action::CloseDocument);
            }
            let tags =
                commands::button_made(ui, "tags.show", state, |command| Button::new(command.name));
            if let Some((_, Some(_))) = tags {
                asked.push(Request::Tags(TagsOf::Entry(document.entry.clone())));
            }
        });
    });
}

/// A structured report: while it is being read, why it could not be, or the lines of it.
fn report(ui: &mut Ui, open: &OpenDocument) {
    match &open.report {
        None => {
            ui.label(RichText::new(READING).color(theme::QUIET_TEXT));
        }
        Some(Err(why)) => {
            ui.label(RichText::new(why).color(theme::WARNING));
        }
        Some(Ok(report)) => {
            ui.label(RichText::new(HEADING).strong());
            if report.cut_short {
                ui.label(RichText::new(CUT_SHORT).color(theme::WARNING));
            }
            ui.add_space(6.0);
            lines(ui, report);
        }
    }
}

/// The lines of a report, in its order, each moved to the right by how many sections it lies
/// in. The text can be selected and copied.
fn lines(ui: &mut Ui, report: &Report) {
    ScrollArea::vertical()
        .id_salt("voxa: the lines of a report")
        .auto_shrink(false)
        .show(ui, |ui| {
            for line in &report.lines {
                ui.horizontal(|ui| {
                    ui.add_space(INDENT * f32::from(line.depth));
                    if let Some(heading) = &line.heading {
                        let words = RichText::new(printable(heading)).strong();
                        ui.add(Label::new(words).wrap());
                    }
                    if !line.text.is_empty() {
                        ui.add(Label::new(printable(&line.text)).wrap());
                    }
                });
            }
        });
}

/// A PDF: it is said to be one, and a copy of it can be saved.
fn pdf(ui: &mut Ui, entry: &str, asked: &mut Vec<Request>) {
    ui.label(IS_A_PDF);
    if ui.button(SAVE_PDF).clicked() {
        asked.push(Request::SaveDocumentPdf(entry.to_owned()));
    }
}

#[cfg(test)]
mod tests;
