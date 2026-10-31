//! Reading a report and saving a PDF on a worker, and listing the tags of a named entry.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc::channel;
use std::sync::{Arc, Barrier};
use std::time::Duration;

use tempfile::TempDir;
use voxa_core::VoxaError;
use voxa_dicom::testkit::{DocumentOptions, ReportItem, structured_report};
use voxa_dicom::{Entry, Index, Report, ReportLine, SeriesSource};

use super::super::lock;
use super::super::tests::{Bench, folder_of};
use crate::action::{Action, Effect};
use crate::state::NoticeKind;
use crate::testing::documents::{NOTE, PDF, REPORT, document_files, page};
use crate::testing::three_series_files;

/// An export of the made-up documents alone, in a folder, and its index.
fn documents_in_a_folder(bench: &Bench) -> (TempDir, Index) {
    let folder = folder_of(&document_files());
    let index = bench
        .index(folder.path())
        .expect("an export of documents is read");
    (folder, index)
}

fn read(bench: &Bench, index: &Index, entry: &str) -> Result<Report, String> {
    let asked = Effect::ReadDocument {
        entry: entry.to_owned(),
    };
    bench.workers.run(asked, Some(index));
    match bench.next() {
        Action::DocumentRead { entry: of, report } => {
            assert_eq!(of, entry);
            report
        }
        other => panic!("unexpected {other:?}"),
    }
}

fn save(bench: &Bench, index: &Index, entry: &str, to: &Path) -> crate::state::Notice {
    let asked = Effect::SaveDocumentPdf {
        entry: entry.to_owned(),
        to: to.to_owned(),
    };
    bench.workers.run(asked, Some(index));
    match bench.next() {
        Action::Post(notice) => notice,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn an_export_of_documents_alone_is_read_not_refused() {
    let bench = Bench::with_budget(1 << 20);
    let (_folder, index) = documents_in_a_folder(&bench);
    assert_eq!(index.series.len(), 0);
    assert_eq!(index.documents.len(), 3);
    assert!(bench.is_quiet(), "and nothing follows the end of indexing");
}

#[test]
fn an_export_with_neither_images_nor_documents_is_still_refused() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&[("notes/readme.txt".to_owned(), b"hello".to_vec())]);
    let refused = bench.index(folder.path()).expect_err("nothing to show");
    assert!(refused.starts_with("No images were found"), "{refused}");
}

#[test]
fn a_report_is_read_on_a_worker_as_its_lines() {
    let bench = Bench::with_budget(1 << 20);
    let (_folder, index) = documents_in_a_folder(&bench);
    let report = read(&bench, &index, REPORT).expect("a report is read");
    assert!(!report.cut_short);
    let heads: Vec<_> = report
        .lines
        .iter()
        .map(|line| line.heading.as_deref())
        .collect();
    assert_eq!(heads, [Some("Part A"), None, None, None]);
    assert_eq!(report.lines[2].text, "Width: 12 mm");
    assert_eq!((report.lines[1].depth, report.lines[3].depth), (1, 0));
    assert!(bench.is_quiet(), "it is said once");
}

#[test]
fn a_report_that_cannot_be_read_is_said_in_the_windows_own_words() {
    let bench = Bench::with_budget(1 << 20);
    let mut files = document_files();
    let damaged = files[0].1[..files[0].1.len() / 2].to_vec();
    files.push(("DOCS/D0001".to_owned(), damaged));
    let folder = folder_of(&files);
    let index = bench.index(folder.path()).expect("indexes");
    let sentence = "This report could not be read. The file may be damaged.";
    // Cut short in the middle, not a report at all, and not in the export.
    for entry in ["DOCS/D0001", PDF, NOTE, "DOCS/nothing-like-it"] {
        let said = read(&bench, &index, entry).expect_err(entry);
        assert!(!said.contains("decoded"), "{entry}: {said}");
        assert!(said.starts_with("This report could not be read"), "{said}");
        assert!(said.ends_with('.'), "{said}");
    }
    let said = read(&bench, &index, "DOCS/D0001").expect_err("damaged");
    assert_eq!(said, sentence);
}

/// A source whose every read waits at `gate` twice: once on coming in, and once more before it
/// gives its bytes.
struct Gated {
    gate: Arc<Barrier>,
    bytes: Vec<u8>,
}

impl SeriesSource for Gated {
    fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VoxaError>> + '_> {
        Box::new(std::iter::empty())
    }

    fn read(&mut self, _: &str) -> Result<Vec<u8>, VoxaError> {
        self.gate.wait();
        self.gate.wait();
        Ok(self.bytes.clone())
    }
}

#[test]
fn a_report_read_for_an_export_since_replaced_is_not_reported() {
    let bench = Bench::with_budget(1 << 20);
    let gate = Arc::new(Barrier::new(2));
    let bytes = document_files().remove(0).1;
    let gated = Gated {
        gate: Arc::clone(&gate),
        bytes,
    };
    *lock(&bench.workers.shared.source) = Some(Box::new(gated));
    let asked = Effect::ReadDocument {
        entry: REPORT.to_owned(),
    };
    bench.workers.run(asked, None);
    gate.wait(); // the read has begun
    bench.workers.shared.opened.fetch_add(1, Ordering::SeqCst); // another export is opened
    gate.wait(); // and the read ends
    assert!(bench.is_quiet(), "the old export's report is not reported");
}

#[test]
fn a_pdf_is_written_where_it_was_chosen_and_nowhere_else() {
    let bench = Bench::with_budget(1 << 20);
    let (_folder, index) = documents_in_a_folder(&bench);
    let chosen = tempfile::tempdir().expect("a temporary folder");
    let to = chosen.path().join("a copy.pdf");
    let notice = save(&bench, &index, PDF, &to);
    assert_eq!(notice.kind, NoticeKind::Info);
    assert_eq!(notice.text, "Saved a copy.pdf");
    assert_eq!(
        std::fs::read(&to).expect("written"),
        page(),
        "the PDF, whole"
    );
    let names: Vec<_> = std::fs::read_dir(chosen.path())
        .expect("lists")
        .map(|file| file.expect("a file").file_name())
        .collect();
    assert_eq!(names.len(), 1, "nothing else is written: {names:?}");
    assert!(bench.is_quiet());
}

#[test]
fn a_pdf_that_cannot_be_saved_says_why_and_writes_nothing() {
    let bench = Bench::with_budget(1 << 20);
    let (_folder, index) = documents_in_a_folder(&bench);
    let chosen = tempfile::tempdir().expect("a temporary folder");
    let nowhere = chosen.path().join("no such folder").join("copy.pdf");
    let notice = save(&bench, &index, PDF, &nowhere);
    assert_eq!(notice.kind, NoticeKind::Error);
    assert!(
        notice.text.starts_with("The file could not be written"),
        "{}",
        notice.text
    );
    // Not a PDF, or not in the export: the person is told, and nothing is written.
    for entry in [REPORT, "DOCS/nothing-like-it"] {
        let to = chosen.path().join("copy.pdf");
        let notice = save(&bench, &index, entry, &to);
        assert_eq!(notice.kind, NoticeKind::Error, "{entry}");
        assert!(notice.text.contains("PDF"), "{}", notice.text);
        assert!(!to.exists(), "{entry}");
    }
}

#[test]
fn the_tags_of_a_named_entry_are_listed_on_a_worker() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&[three_series_files(), document_files()].concat());
    let index = bench.index(folder.path()).expect("indexes");
    assert_eq!(index.documents.len(), 3);
    let (send, rows) = channel();
    bench
        .workers
        .list_tags_of_entry(REPORT.to_owned(), move |listed| {
            let _ = send.send(listed);
        });
    let listed = rows
        .recv_timeout(Duration::from_secs(60))
        .expect("answered");
    let listed = listed.expect("the tags are listed");
    let keywords: Vec<_> = listed.iter().map(|row| row.keyword.as_str()).collect();
    assert!(keywords.contains(&"SOPClassUID"), "{keywords:?}");
    assert!(keywords.contains(&"ContentSequence"), "{keywords:?}");
    // An entry that is not there is an error in plain words, not a panic.
    let (send, rows) = channel();
    bench
        .workers
        .list_tags_of_entry("DOCS/nothing-like-it".to_owned(), move |listed| {
            let _ = send.send(listed);
        });
    let missing = rows
        .recv_timeout(Duration::from_secs(60))
        .expect("answered");
    assert!(missing.is_err());
}

#[test]
fn a_report_is_cleared_of_characters_that_turn_or_hide_text_once_where_it_is_read() {
    // Read from a file, a line break and a tab become spaces.
    let bench = Bench::with_budget(1 << 20);
    let content = [ReportItem::text("Note", "one\ttwo\nthree")];
    let opts = DocumentOptions::default();
    let files = vec![("DOCS/A0001".to_owned(), structured_report(&opts, &content))];
    let folder = folder_of(&files);
    let index = bench.index(folder.path()).expect("indexes");
    let report = read(&bench, &index, "DOCS/A0001").expect("a report is read");
    assert_eq!(report.lines[0].text, "Note: one two three");
    // And the marks that turn or hide letters are left out, wherever a line has them.
    let line = |heading: Option<&str>, text: &str, is_name: bool| ReportLine {
        depth: 1,
        heading: heading.map(str::to_owned),
        text: text.to_owned(),
        is_name,
    };
    let turned = Report {
        lines: vec![
            line(Some("Se\u{202E}ction"), "", false),
            line(None, "No\u{200B}te: one\u{202E}two\u{2066}three", false),
            line(None, "Seen by: Nobody\u{200F}^Madeup", true),
        ],
        cut_short: true,
    };
    let cleared = super::printable_lines(turned);
    assert!(cleared.cut_short);
    assert_eq!(
        cleared.lines,
        [
            line(Some("Section"), "", false),
            line(None, "Note: onetwothree", false),
            line(None, "Seen by: Nobody^Madeup", true),
        ]
    );
}
