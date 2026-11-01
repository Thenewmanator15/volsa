//! Work on the documents of the export that is open, off the interface's thread: reading a
//! structured report as text, saving a PDF stored in DICOM as the PDF it was, and listing the
//! tags of a file by its entry. A report is only ever turned into text, and a PDF is only ever
//! written, as the bytes it holds, to the place the person chose: nothing is opened, run or
//! shown by another program.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use voxa_core::VoxaError;
use voxa_dicom::{Report, ReportLine, SliceRef, TagRow, pdf_bytes, read_report};

use super::{Shared, Workers};
use crate::action::{Action, DocumentAction};
use crate::naming::{printable, printable_multiline};
use crate::state::{Notice, NoticeKind};

/// What is said of a report that is there but cannot be read as one.
const NOT_READ: &str = "This report could not be read. The file may be damaged.";
/// What is said of a report that cannot be got out of the export.
const NOT_FOUND: &str = "This report could not be read. It is not in the export any more.";
/// What is said when the PDF in a file cannot be got out of it.
const NO_PDF: &str = "A copy of the PDF could not be made. The file may be damaged.";
/// The key of the notices about saving a file: each takes the place of the one before.
const SAVING: &str = "saving";

impl Workers {
    /// Read the structured report in `entry` of the export that is open as lines of text, and
    /// report it, or the sentence that says why it could not be read. Nothing is reported for
    /// an export that has since been replaced.
    pub(super) fn read_document(&self, entry: String) {
        let shared = Arc::clone(&self.shared);
        let opened = shared.opened.load(Ordering::SeqCst);
        self.spawn(move || {
            let report = shared.report_in(&entry);
            if shared.opened.load(Ordering::SeqCst) == opened {
                shared.report(Action::Document(DocumentAction::Read { entry, report }));
            }
        });
    }

    /// Write the PDF stored in `entry` of the export that is open to `to`, and say how it
    /// went. The file is written where the person chose and nowhere else.
    pub(super) fn save_document_pdf(&self, entry: String, to: PathBuf) {
        let shared = Arc::clone(&self.shared);
        let otherwise = notice(NoticeKind::Error, NO_PDF.to_owned());
        self.post(otherwise, move || shared.save_pdf(&entry, &to));
    }

    /// List the elements of the file in `entry` of the export that is open, on a worker, and
    /// hand them to `reply`, as [`Workers::list_tags`] does for the file behind a slice.
    pub fn list_tags_of_entry(
        &self,
        entry: String,
        reply: impl FnOnce(Result<Vec<TagRow>, String>) + Send + 'static,
    ) {
        self.list_tags_in(SliceRef { entry, frame: None }, reply);
    }
}

impl Shared {
    /// The report in `entry` of the export that is open, or the sentence that says why it
    /// cannot be had. The log says what kind of failure it was, never which file.
    fn report_in(&self, entry: &str) -> Result<Report, String> {
        let bytes = match self.read_entry(entry) {
            Some(Ok(bytes)) => bytes,
            Some(Err(error)) => return Err(logged(&error, NOT_FOUND)),
            None => return Err(NOT_FOUND.to_owned()),
        };
        read_report(&bytes)
            .map(printable_lines)
            .map_err(|error| logged(&error, NOT_READ))
    }

    /// Write the PDF in `entry` to `to`, and say how it went.
    fn save_pdf(&self, entry: &str, to: &Path) -> Notice {
        let stored = match self.read_entry(entry) {
            Some(read) => read.and_then(|bytes| pdf_bytes(&bytes)),
            None => Err(VoxaError::MissingEntry(entry.to_owned())),
        };
        match stored {
            Err(error) => notice(NoticeKind::Error, logged(&error, NO_PDF)),
            Ok(pdf) => match crate::export::write(to, &pdf) {
                Ok(()) => notice(NoticeKind::Info, format!("Saved {}", name_of(to))),
                Err(why) => notice(NoticeKind::Error, why.to_string()),
            },
        }
    }
}

/// Log what kind of failure `error` was, which says nothing of the file, and give the
/// sentence to say of it.
fn logged(error: &VoxaError, sentence: &str) -> String {
    tracing::warn!("a document could not be read: {}", error.kind());
    sentence.to_owned()
}

/// The report with every line cleared once, here, of the characters that would turn, hide or
/// break the words around them, so that the window draws the lines as they are and does no
/// such work for each line each time it draws. The text of a line keeps its own line breaks,
/// which the window draws; a heading is one line.
fn printable_lines(report: Report) -> Report {
    let lines = report.lines.into_iter().map(|line| ReportLine {
        heading: line.heading.map(|heading| printable(&heading)),
        text: printable_multiline(&line.text),
        ..line
    });
    Report {
        lines: lines.collect(),
        cut_short: report.cut_short,
    }
}

/// A notice about saving a PDF.
fn notice(kind: NoticeKind, text: String) -> Notice {
    Notice {
        kind,
        text,
        details: Vec::new(),
        key: Some(SAVING),
    }
}

/// The name of the file the person chose, without the folders it is in.
fn name_of(path: &Path) -> String {
    let name = path.file_name();
    name.map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
