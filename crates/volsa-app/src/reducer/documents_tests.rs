//! Documents in the reducer: opening one, taking in what a worker read of it, closing it, and
//! the one piece of work saving a PDF asks for.

use std::path::PathBuf;

use volsa_dicom::{Report, ReportLine};

use super::*;
use crate::state::OpenDocument;
use crate::testing::documents::{NOTE, PDF, REPORT, documents_alone, scans_and_documents};
use crate::testing::{LOCALISER, THICK_SCAN, id, odd_series_files};

/// A state that has read an export of three scans and three documents, with nothing shown.
fn with_documents() -> AppState {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(scans_and_documents()));
    state
}

fn open(state: &mut AppState, entry: &str) -> Vec<Effect> {
    reduce(
        state,
        Action::Document(DocumentAction::Open(entry.to_owned())),
    )
}

fn a_report() -> Report {
    Report {
        lines: vec![ReportLine {
            depth: 0,
            heading: None,
            text: "Comment: A made-up remark.".to_owned(),
            is_name: false,
        }],
        cut_short: false,
    }
}

fn read(entry: &str, report: Result<Report, String>) -> Action {
    Action::Document(DocumentAction::Read {
        entry: entry.to_owned(),
        report,
    })
}

fn read_effect(entry: &str) -> Effect {
    Effect::ReadDocument {
        entry: entry.to_owned(),
    }
}

#[test]
fn opening_a_report_asks_a_worker_to_read_it() {
    let mut state = with_documents();
    assert_eq!(open(&mut state, REPORT), [read_effect(REPORT)]);
    let waiting = OpenDocument {
        entry: REPORT.to_owned(),
        report: None,
    };
    assert_eq!(state.document, Some(waiting));
}

#[test]
fn opening_a_pdf_or_a_file_volsa_cannot_show_reads_nothing() {
    for entry in [PDF, NOTE] {
        let mut state = with_documents();
        assert_eq!(open(&mut state, entry), [], "{entry}");
        let shown = state.document.as_ref().map(|open| open.entry.as_str());
        assert_eq!(shown, Some(entry));
    }
}

#[test]
fn a_document_that_is_not_in_the_export_is_not_opened() {
    let mut state = with_documents();
    assert_eq!(open(&mut state, "DOCS/nothing-like-it"), []);
    assert_eq!(state.document, None);
    // And so is a scan's own file.
    let mut state = AppState::default();
    assert_eq!(open(&mut state, REPORT), [], "no export is open");
    assert_eq!(state.document, None);
}

#[test]
fn what_a_worker_read_is_kept_for_the_document_still_open() {
    let mut state = with_documents();
    open(&mut state, REPORT);
    reduce(&mut state, read(REPORT, Ok(a_report())));
    let kept = state.document.as_ref().and_then(|open| open.report.clone());
    assert_eq!(kept, Some(Ok(a_report())));
    // A report that could not be read is kept as the sentence that says so.
    let mut state = with_documents();
    open(&mut state, REPORT);
    reduce(
        &mut state,
        read(REPORT, Err("It could not be read.".to_owned())),
    );
    let kept = state.document.as_ref().and_then(|open| open.report.clone());
    assert_eq!(kept, Some(Err("It could not be read.".to_owned())));
}

#[test]
fn a_report_read_for_a_document_no_longer_open_is_ignored() {
    let mut state = with_documents();
    open(&mut state, REPORT);
    open(&mut state, PDF);
    let before = state.clone();
    reduce(&mut state, read(REPORT, Ok(a_report())));
    assert_eq!(state, before, "the PDF open now is not given a report");
    reduce(&mut state, Action::Document(DocumentAction::Close));
    reduce(&mut state, read(REPORT, Ok(a_report())));
    assert_eq!(state.document, None, "and nothing is opened by a report");
}

#[test]
fn opening_the_report_already_open_reads_it_again_only_if_it_could_not_be_read() {
    let mut state = with_documents();
    open(&mut state, REPORT);
    reduce(&mut state, read(REPORT, Ok(a_report())));
    assert_eq!(open(&mut state, REPORT), [], "it is shown, and kept");
    assert!(matches!(&state.document, Some(open) if open.report.is_some()));
    reduce(&mut state, read(REPORT, Err("Not read.".to_owned())));
    assert_eq!(
        open(&mut state, REPORT),
        [read_effect(REPORT)],
        "tried again"
    );
}

#[test]
fn choosing_a_scan_closes_the_document() {
    let mut state = with_documents();
    open(&mut state, REPORT);
    reduce(&mut state, read(REPORT, Ok(a_report())));
    let series = id(THICK_SCAN);
    let shown = Action::SelectSeries {
        viewport: 0,
        series,
    };
    reduce(&mut state, shown);
    assert_eq!(state.document, None);
    assert_eq!(state.viewports[0].series, Some(id(THICK_SCAN)));
    // A series that is not there is no choice, and leaves the document open.
    open(&mut state, PDF);
    let nothing = Action::SelectSeries {
        viewport: 0,
        series: id("1.2.3.99"),
    };
    reduce(&mut state, nothing);
    assert!(state.document.is_some());
}

#[test]
fn closing_the_document_closes_it_and_another_export_starts_with_none() {
    let mut state = with_documents();
    open(&mut state, PDF);
    reduce(&mut state, Action::Document(DocumentAction::Close));
    assert_eq!(state.document, None);
    open(&mut state, PDF);
    reduce(&mut state, Action::OpenPath(PathBuf::from("made-up.zip")));
    assert_eq!(state.document, None);
}

#[test]
fn a_pdf_is_saved_by_one_piece_of_work_and_nothing_else() {
    let mut state = with_documents();
    open(&mut state, PDF);
    let before = state.clone();
    let to = PathBuf::from("chosen.pdf");
    let save = Action::Document(DocumentAction::SavePdf(PDF.to_owned(), to.clone()));
    let effects = reduce(&mut state, save);
    let saving = Effect::SaveDocumentPdf {
        entry: PDF.to_owned(),
        to: to.clone(),
    };
    assert_eq!(effects, [saving], "the only work is writing the copy");
    assert_eq!(state, before, "and the state does not change");
    // Only a PDF has a copy to save.
    for entry in [REPORT, NOTE, "DOCS/nothing-like-it"] {
        let save = Action::Document(DocumentAction::SavePdf(entry.to_owned(), to.clone()));
        assert_eq!(reduce(&mut state, save), [], "{entry}");
    }
}

#[test]
fn an_export_of_documents_alone_opens_the_first_of_them() {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(documents_alone()));
    let effects = reduce(&mut state, Action::IndexDone);
    let shown = state.document.as_ref().map(|open| open.entry.as_str());
    assert_eq!(shown, Some(REPORT), "the first the rail lists");
    assert!(effects.contains(&read_effect(REPORT)), "{effects:?}");
    assert_eq!(state.viewports[0].series, None);
}

#[test]
fn any_series_opens_before_a_document() {
    // Scans first.
    let mut state = with_documents();
    reduce(&mut state, Action::IndexDone);
    assert_eq!(state.document, None);
    assert!(state.viewports[0].series.is_some());
    // And with only locating images among the series, they open, not the documents.
    let locating = odd_series_files()
        .into_iter()
        .filter(|(name, _)| name.starts_with("S4/"));
    let files = [
        locating.collect(),
        crate::testing::documents::document_files(),
    ]
    .concat();
    let mut state = AppState::default();
    reduce(
        &mut state,
        Action::IndexProgress(crate::testing::indexed(files)),
    );
    reduce(&mut state, Action::IndexDone);
    assert_eq!(state.viewports[0].series, Some(id(LOCALISER)));
    assert_eq!(state.document, None);
}
