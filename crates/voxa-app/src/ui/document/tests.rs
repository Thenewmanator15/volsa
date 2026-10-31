//! A document in the middle of the window, drawn with no window behind it: a report is headed
//! and shown as lines, a PDF offers a copy, any other file is named, and each offers its tags.

use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use egui::{Context, Event, Pos2, Rect, vec2};
use voxa_dicom::{Report, ReportLine};

use super::*;
use crate::action::Effect;
use crate::reducer::reduce;
use crate::session::Session;
use crate::testing::documents::{
    NOTE, PDF, PDF_TITLE, REPORT, REPORT_TITLE, document_files, scans_and_documents,
};
use crate::testing::three_series_files;
use crate::ui::seen::{self, Seen, click_at};

/// The window these passes are drawn in.
pub(super) const WINDOW: [f32; 2] = [900.0, 600.0];

/// What one pass of the document gave and drew.
pub(super) struct Drawn {
    pub actions: Vec<Action>,
    pub asked: Vec<Request>,
    pub seen: Seen,
}

/// A state that has read the made-up export, with this document open.
pub(super) fn opened(entry: &str, report: Option<Result<Report, String>>) -> AppState {
    opened_with(scans_and_documents(), entry, report)
}

/// A state that has read this export, with this document open.
pub(super) fn opened_with(
    index: voxa_dicom::Index,
    entry: &str,
    report: Option<Result<Report, String>>,
) -> AppState {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(index));
    let entry = entry.to_owned();
    state.document = Some(OpenDocument { entry, report });
    state
}

/// One pass of the window with what is kept of the report from before.
pub(super) fn pass_with(
    ctx: &Context,
    state: &AppState,
    kept: &mut Kept,
    events: Vec<Event>,
) -> Drawn {
    let open = state.document.as_ref().expect("a document is open");
    let mut actions = Vec::new();
    let draw = |ui: &mut Ui| {
        let area = Rect::from_min_size(Pos2::ZERO, vec2(WINDOW[0], WINDOW[1]));
        show(ui, area, state, open, kept, &mut actions)
    };
    let (asked, seen) = seen::pass(ctx, WINDOW, events, draw);
    Drawn {
        actions,
        asked,
        seen,
    }
}

/// One pass of the window with nothing kept from before.
pub(super) fn pass(ctx: &Context, state: &AppState, events: Vec<Event>) -> Drawn {
    pass_with(ctx, state, &mut Kept::default(), events)
}

fn click(ctx: &Context, state: &AppState, name: &str) -> Drawn {
    let drawn = pass(ctx, state, Vec::new());
    let at = drawn.seen.place_of(name).expect("a button of that name");
    let [press, release] = click_at(at.center());
    let _ = pass(ctx, state, press);
    pass(ctx, state, release)
}

pub(super) fn line(depth: u8, heading: Option<&str>, text: &str) -> ReportLine {
    ReportLine {
        depth,
        heading: heading.map(str::to_owned),
        text: text.to_owned(),
        is_name: false,
    }
}

/// A report of three levels, as the reader of the made-up report gives it, and one more.
fn a_report(cut_short: bool) -> Report {
    Report {
        lines: vec![
            line(0, Some("Part A"), ""),
            line(1, None, "Comment: A made-up remark."),
            line(2, None, "Width: 12 mm"),
            line(0, None, "Closing: That is all."),
        ],
        cut_short,
    }
}

pub(super) fn words_of(seen: &Seen) -> Vec<&str> {
    seen.words.iter().map(|(word, _)| word.as_str()).collect()
}

pub(super) fn where_is(seen: &Seen, word: &str) -> Rect {
    let found = seen.words.iter().find(|(drawn, _)| drawn == word);
    found.map_or_else(|| panic!("{word}: {:?}", words_of(seen)), |(_, at)| *at)
}

/// A folder holding these files.
fn folder_of(files: &[(String, Vec<u8>)]) -> tempfile::TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder");
    for (name, bytes) in files {
        let path = folder.path().join(name);
        std::fs::create_dir_all(path.parent().expect("inside the folder")).expect("makes folders");
        std::fs::write(path, bytes).expect("writes");
    }
    folder
}

/// Take in what the workers report until `done` holds.
fn until(session: &mut Session, done: impl Fn(&Session) -> bool) {
    let started = Instant::now();
    while !done(session) {
        assert!(started.elapsed() < Duration::from_secs(60), "still waiting");
        session.wait(Duration::from_millis(50));
    }
}

#[test]
fn opening_a_report_reads_it_on_a_worker_and_shows_its_lines() {
    let folder = folder_of(&[three_series_files(), document_files()].concat());
    let (sender, reports) = channel();
    let mut session = Session::new(sender, reports, 1 << 24, None);
    session.dispatch(Action::OpenPath(folder.path().to_owned()));
    until(&mut session, |session| {
        session.state().viewports[0].series.is_some()
    });
    session.dispatch(Action::OpenDocument(REPORT.to_owned()));
    // Asked for, not yet read: the window says it is reading, and has no lines to show.
    let open = session.state().document.clone().expect("open");
    assert_eq!(open.report, None);
    let ctx = seen::context(1.0);
    let waiting = pass(&ctx, session.state(), Vec::new());
    let words = words_of(&waiting.seen);
    assert!(words.contains(&READING), "{words:?}");
    until(&mut session, |session| {
        let open = session.state().document.as_ref();
        open.is_some_and(|open| open.report.is_some())
    });
    let drawn = pass(&ctx, session.state(), Vec::new());
    let words = words_of(&drawn.seen);
    let lines = [
        "Part A",
        "Comment: A made-up remark.",
        "Width: 12 mm",
        "Closing: That is all.",
    ];
    for shown in lines {
        assert!(words.contains(&shown), "{shown}: {words:?}");
    }
    assert!(!words.contains(&READING));
    // In the report's order, one under the other.
    let tops: Vec<f32> = lines
        .iter()
        .map(|word| where_is(&drawn.seen, word).top())
        .collect();
    assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
}

#[test]
fn the_report_is_headed_as_not_the_reference() {
    let state = opened(REPORT, Some(Ok(a_report(false))));
    let drawn = pass(&seen::context(1.0), &state, Vec::new());
    // The spec's words, exactly.
    assert_eq!(
        HEADING,
        "Shown as text. The original report is the reference."
    );
    let heading = where_is(&drawn.seen, HEADING);
    let first = where_is(&drawn.seen, "Part A");
    assert!(
        heading.bottom() <= first.top(),
        "{heading:?} over {first:?}"
    );
    // A report that could not be read is not headed as one that is shown.
    let state = opened(REPORT, Some(Err("It could not be read.".to_owned())));
    let drawn = pass(&seen::context(1.0), &state, Vec::new());
    assert!(!words_of(&drawn.seen).contains(&HEADING));
    assert!(words_of(&drawn.seen).contains(&"It could not be read."));
}

#[test]
fn a_cut_short_report_says_so() {
    assert_eq!(
        CUT_SHORT,
        "Part of this report is too long or too deeply nested to show here."
    );
    let ctx = seen::context(1.0);
    let cut = pass(&ctx, &opened(REPORT, Some(Ok(a_report(true)))), Vec::new());
    let words = words_of(&cut.seen);
    assert!(words.contains(&CUT_SHORT), "{words:?}");
    let whole = pass(&ctx, &opened(REPORT, Some(Ok(a_report(false)))), Vec::new());
    assert!(!words_of(&whole.seen).contains(&CUT_SHORT));
}

#[test]
fn lines_are_indented_by_how_deep_they_lie_and_the_text_can_be_selected() {
    let ctx = seen::context(1.0);
    let drawn = pass(&ctx, &opened(REPORT, Some(Ok(a_report(false)))), Vec::new());
    let left = |word: &str| where_is(&drawn.seen, word).left();
    let (outer, middle, inner) = (
        left("Closing: That is all."),
        left("Comment: A made-up remark."),
        left("Width: 12 mm"),
    );
    assert!((middle - outer - INDENT).abs() < 0.5, "{outer} {middle}");
    assert!((inner - middle - INDENT).abs() < 0.5, "{middle} {inner}");
    // Labels of this window can be selected and copied, and those of the report are not set
    // otherwise.
    assert!(ctx.global_style().interaction.selectable_labels);
}

#[test]
fn the_header_names_the_kind_the_title_and_the_date() {
    let ctx = seen::context(1.0);
    let drawn = pass(&ctx, &opened(REPORT, None), Vec::new());
    let words = words_of(&drawn.seen);
    assert!(words.contains(&"Report"), "{words:?}");
    let said = format!("{REPORT_TITLE} · 2 January 2020");
    assert!(words.contains(&said.as_str()), "{words:?}");
}

#[test]
fn a_pdf_offers_a_copy_and_starts_no_program() {
    let state = opened(PDF, None);
    let ctx = seen::context(1.0);
    let drawn = pass(&ctx, &state, Vec::new());
    assert_eq!(IS_A_PDF, "This report is a PDF. Voxa does not show PDFs.");
    assert_eq!(SAVE_PDF, "Save a copy as PDF…");
    let words = words_of(&drawn.seen);
    assert!(words.contains(&IS_A_PDF), "{words:?}");
    let said = format!("{PDF_TITLE} · 2 January 2020");
    assert!(words.contains(&said.as_str()), "{words:?}");
    assert!(!words.contains(&HEADING), "a PDF is not shown as text");
    // The button asks the window for a place to save, and for nothing else: no state changes
    // and no program starts.
    let clicked = click(&ctx, &state, SAVE_PDF);
    assert_eq!(clicked.asked, [Request::SaveDocumentPdf(PDF.to_owned())]);
    assert_eq!(clicked.actions, []);
    // Once a place is chosen the one piece of work is writing the copy there.
    let chosen = PathBuf::from("chosen.pdf");
    let mut state = state;
    let save = Action::SaveDocumentPdf(PDF.to_owned(), chosen.clone());
    let effects = reduce(&mut state, save);
    let writing = Effect::SaveDocumentPdf {
        entry: PDF.to_owned(),
        to: chosen,
    };
    assert_eq!(effects, [writing]);
}

#[test]
fn voxa_has_no_code_that_starts_another_program() {
    // What S-UI-01 and UI-35 promise: a PDF is only ever written, never opened. The needles are
    // written in pieces so that this file does not hold them.
    let needles = [
        ["process::", "Command"].concat(),
        ["Command", "::new"].concat(),
        ["open", "::that"].concat(),
    ];
    let mut folders = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).expect("lists the sources") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                folders.push(path);
                continue;
            }
            if path.extension().is_none_or(|ending| ending != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("reads a source");
            for needle in &needles {
                let holds = text.contains(needle.as_str());
                assert!(!holds, "{} holds {needle}", path.display());
            }
        }
    }
}

#[test]
fn another_kind_of_file_is_named_and_said_to_be_one_voxa_cannot_show() {
    let ctx = seen::context(1.0);
    let drawn = pass(&ctx, &opened(NOTE, None), Vec::new());
    assert_eq!(CANNOT_SHOW, "Voxa cannot show this kind of file.");
    let words = words_of(&drawn.seen);
    assert!(words.contains(&"Key-image note"), "{words:?}");
    assert!(words.contains(&CANNOT_SHOW));
    let no_pdf = drawn.seen.place_of(SAVE_PDF).is_none();
    assert!(no_pdf, "there is no PDF to save");
}

#[test]
fn each_kind_offers_the_tags_of_its_file_and_a_way_to_close() {
    let tags = commands::find("tags.show").expect("listed").name;
    let ctx = seen::context(1.0);
    for entry in [REPORT, PDF, NOTE] {
        let state = opened(entry, None);
        let asked = click(&ctx, &state, tags);
        let of = TagsOf::Entry(entry.to_owned());
        assert_eq!(asked.asked, [Request::Tags(of)], "{entry}");
        assert_eq!(asked.actions, []);
        let closed = click(&ctx, &state, CLOSE);
        assert_eq!(closed.actions, [Action::CloseDocument], "{entry}");
        assert_eq!(closed.asked, []);
    }
}
