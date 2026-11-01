//! What can happen to the document open in the place of the pictures: a report or a PDF
//! stored in the export.

use std::path::PathBuf;

use volsa_dicom::Report;

/// Something about the document open in the place of the pictures.
#[derive(Debug, Clone, PartialEq)]
pub enum DocumentAction {
    /// Open a document of the export, by the name of its entry, in the middle of the window.
    Open(String),
    /// Close the document that is open.
    Close,
    /// A worker has read a structured report as text, or could not.
    Read {
        /// The entry the report was read from.
        entry: String,
        /// The report, or the sentence that says why it could not be read.
        report: Result<Report, String>,
    },
    /// Save the PDF stored in the document with this entry name (the first) to the place the
    /// person chose (the second).
    SavePdf(String, PathBuf),
}
