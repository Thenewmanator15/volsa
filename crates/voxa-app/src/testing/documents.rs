//! Made-up documents for the tests: a structured report, a PDF stored in DICOM and a key-image
//! note, and exports that hold them.

use voxa_dicom::Index;
use voxa_dicom::testkit::{DocumentOptions, ReportItem, encapsulated_pdf, structured_report};

use super::{indexed, three_series_files};

/// The entry of the made-up report.
pub const REPORT: &str = "DOCS/A0001";
/// The entry of the made-up PDF.
pub const PDF: &str = "DOCS/B0001";
/// The entry of the made-up key-image note, which Voxa lists but cannot show.
pub const NOTE: &str = "DOCS/C0001";
/// The title of the made-up report.
pub const REPORT_TITLE: &str = "Made-up report";
/// The title of the made-up PDF.
pub const PDF_TITLE: &str = "Made-up page";
/// The SOP class of a key-image note.
const KEY_IMAGE_NOTE: &str = "1.2.840.10008.5.1.4.1.1.88.59";

/// The options of one made-up document, which is in the study of the default slice.
fn options(title: &str, instance_number: i32) -> DocumentOptions {
    DocumentOptions {
        title: title.to_owned(),
        instance_number,
        ..DocumentOptions::default()
    }
}

/// The content of the made-up report: a section with a text and a number, then a closing text.
pub fn report_content() -> Vec<ReportItem> {
    vec![
        ReportItem::container(
            "Part A",
            vec![
                ReportItem::text("Comment", "A made-up remark."),
                ReportItem::number("Width", "12", "mm"),
            ],
        ),
        ReportItem::text("Closing", "That is all."),
    ]
}

/// The bytes of the PDF the made-up PDF holds: a short page with every kind of byte in it, of
/// odd length.
pub fn page() -> Vec<u8> {
    let mut page = b"%PDF-1.4\nA made-up page\n".to_vec();
    page.extend_from_slice(&[0, 1, 2, 250, 255]);
    page.extend_from_slice(b"\n%%EOF\n.");
    page
}

/// The files of the made-up report, PDF and key-image note.
pub fn document_files() -> Vec<(String, Vec<u8>)> {
    let note = DocumentOptions {
        sop_class: Some(KEY_IMAGE_NOTE.to_owned()),
        ..options("", 3)
    };
    vec![
        (
            REPORT.to_owned(),
            structured_report(&options(REPORT_TITLE, 1), &report_content()),
        ),
        (
            PDF.to_owned(),
            encapsulated_pdf(&options(PDF_TITLE, 2), &page()),
        ),
        (NOTE.to_owned(), structured_report(&note, &[])),
    ]
}

/// The three made-up scans and the made-up documents, all of one study.
pub fn scans_and_documents() -> Index {
    indexed([three_series_files(), document_files()].concat())
}

/// An export that holds the made-up documents and no image at all.
pub fn documents_alone() -> Index {
    indexed(document_files())
}
