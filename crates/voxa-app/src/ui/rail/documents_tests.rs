//! Documents in the rail: listed with the other files, counted, opened by a click, and kept
//! with the rows from one frame to the next.

use voxa_dicom::{DocumentKind, IndexedDocument};

use super::card_tests::right_click;
use super::document_card::{self, NO_TITLE, OPEN};
use super::tests::{click, pass, top_of};
use super::*;
use crate::action::Action;
use crate::reducer::reduce;
use crate::testing::documents::{
    NOTE, PDF, REPORT, document_files, documents_alone, scans_and_documents,
};
use crate::testing::{SINGLE, id, indexed, series_files};
use crate::ui::commands::TagsOf;
use crate::ui::seen::{self, Seen};

/// An export of one single-slice scan and the made-up documents.
fn one_scan_and_documents() -> AppState {
    let files = [series_files(SINGLE, 5, 1, 4), document_files()].concat();
    AppState {
        index: Some(indexed(files)),
        ..AppState::default()
    }
}

fn document_named(state: &AppState, entry: &str) -> IndexedDocument {
    let index = state.index.as_ref().expect("an export is open");
    let found = index
        .documents
        .iter()
        .find(|document| document.entry == entry);
    found.cloned().expect("listed")
}

fn card_of(seen: &Seen, document: &IndexedDocument) -> Rect {
    let name = document_card::card_name(document);
    let found = seen.place_of(&name);
    found.unwrap_or_else(|| panic!("no card {name}: {:?}", seen.names))
}

#[test]
fn documents_are_other_files_after_the_series_and_counted_with_them() {
    let state = one_scan_and_documents();
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let closed = pass(&ctx, &state, &mut rail, Vec::new());
    let heading = closed.seen.place_of("Other files, 3").expect("counted");
    let documents: Vec<IndexedDocument> = [REPORT, PDF, NOTE]
        .iter()
        .map(|entry| document_named(&state, entry))
        .collect();
    for document in &documents {
        let name = document_card::card_name(document);
        assert!(closed.seen.place_of(&name).is_none(), "{name}: closed");
    }
    rail.other_open = true;
    let open = pass(&ctx, &state, &mut rail, Vec::new());
    let scan = &state.series(&id(SINGLE)).expect("listed").info;
    let scan_top = top_of(&open.seen, scan).expect("drawn");
    let tops: Vec<f32> = documents
        .iter()
        .map(|document| card_of(&open.seen, document).top())
        .collect();
    assert!(scan_top < heading.top(), "the scans come first");
    assert!(tops[0] > heading.top(), "{tops:?} under {heading:?}");
    assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
    // Each is called by its kind, its title and its date, nothing else.
    assert_eq!(
        document_card::card_name(&documents[0]),
        "Report, Made-up report"
    );
    assert_eq!(
        document_card::card_name(&documents[1]),
        "Report (PDF), Made-up page"
    );
    let note = document_card::card_name(&documents[2]);
    assert_eq!(note, format!("Key-image note, {NO_TITLE}"));
}

#[test]
fn a_click_on_a_document_opens_it() {
    let state = one_scan_and_documents();
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    rail.other_open = true;
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let report = document_named(&state, REPORT);
    let clicked = click(&ctx, &state, &mut rail, card_of(&drawn.seen, &report));
    assert_eq!(clicked.actions, [Action::OpenDocument(REPORT.to_owned())]);
    assert_eq!(clicked.requests, []);
}

#[test]
fn a_documents_menu_opens_it_or_shows_the_tags_of_its_file() {
    let state = one_scan_and_documents();
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    rail.other_open = true;
    let tags = commands::find("tags.show").expect("listed").name;
    let note = document_named(&state, NOTE);
    let items = [
        (OPEN, (vec![Action::OpenDocument(NOTE.to_owned())], vec![])),
        (
            tags,
            (vec![], vec![Request::Tags(TagsOf::Entry(NOTE.to_owned()))]),
        ),
    ];
    for (item, expected) in items {
        let drawn = pass(&ctx, &state, &mut rail, Vec::new());
        let card = card_of(&drawn.seen, &note);
        let [press, release] = right_click(card.right_bottom() - egui::vec2(6.0, 6.0));
        let _ = pass(&ctx, &state, &mut rail, press);
        let _ = pass(&ctx, &state, &mut rail, release);
        let opened = pass(&ctx, &state, &mut rail, Vec::new());
        let found = opened.seen.place_of(item).expect("in the menu");
        assert!(!card.contains(found.center()), "{found:?} on {card:?}");
        let chosen = click(&ctx, &state, &mut rail, found);
        assert_eq!((chosen.actions, chosen.requests), expected, "{item}");
    }
}

#[test]
fn an_export_of_documents_alone_opens_other_files_and_shows_the_first() {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(documents_alone()));
    reduce(&mut state, Action::IndexDone);
    let open = state.document.as_ref().map(|open| open.entry.as_str());
    assert_eq!(open, Some(REPORT), "the first is shown");
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    assert!(rail.other_open, "the other files are open");
    assert!(drawn.seen.place_of("Other files, 3").is_some());
    let first = document_named(&state, REPORT);
    let shown = drawn.seen.place_of(&document_card::card_name(&first));
    assert!(
        shown.is_some(),
        "its card is in sight: {:?}",
        drawn.seen.names
    );
    // Documents of studies that hold no scans have nothing to be headed under here.
    let headings = drawn.seen.words.iter();
    let unplaced = headings.filter(|(word, _)| word.starts_with("Documents of"));
    assert_eq!(unplaced.count(), 0);
}

#[test]
fn a_document_come_to_be_open_opens_the_other_files_and_they_can_be_closed_again() {
    let mut state = one_scan_and_documents();
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let _ = pass(&ctx, &state, &mut rail, Vec::new());
    assert!(!rail.other_open);
    reduce(&mut state, Action::OpenDocument(PDF.to_owned()));
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    assert!(rail.other_open, "opened as the document comes to be shown");
    let pdf = document_named(&state, PDF);
    assert!(
        drawn
            .seen
            .place_of(&document_card::card_name(&pdf))
            .is_some()
    );
    // The person may close them again while the same document is shown.
    let heading = drawn.seen.place_of("Other files, 3").expect("drawn");
    let _ = click(&ctx, &state, &mut rail, heading);
    let _ = pass(&ctx, &state, &mut rail, Vec::new());
    assert!(!rail.other_open);
}

#[test]
fn the_rows_are_worked_out_again_only_when_what_they_come_from_changes_documents_included() {
    let mut state = one_scan_and_documents();
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = |state: &AppState, rail: &mut RailState| {
        let _ = pass(&ctx, state, rail, Vec::new());
        rail.builds
    };
    assert_eq!(drawn(&state, &mut rail), 1);
    rail.other_open = true;
    assert_eq!(drawn(&state, &mut rail), 2, "the other files opened");
    assert_eq!(
        drawn(&state, &mut rail),
        2,
        "drawn again with nothing changed"
    );
    // A document opened, read, and closed again is no change to the rows.
    reduce(&mut state, Action::OpenDocument(REPORT.to_owned()));
    assert_eq!(drawn(&state, &mut rail), 2);
    reduce(&mut state, Action::CloseDocument);
    assert_eq!(drawn(&state, &mut rail), 2);
    // The index read again is.
    reduce(&mut state, Action::IndexProgress(scans_and_documents()));
    assert_eq!(drawn(&state, &mut rail), 3);
}

#[test]
fn the_documents_of_a_study_without_scans_come_last_under_a_heading_that_says_so() {
    let mut state = one_scan_and_documents();
    let index = state.index.as_mut().expect("open");
    let own = index.studies[0].id.0.clone();
    let document = |entry: &str, study: &str| IndexedDocument {
        entry: entry.to_owned(),
        study_uid: study.to_owned(),
        kind: DocumentKind::Report,
        title: entry.to_owned(),
        date: None,
    };
    // One of the export's own study, one of a study the export holds no scan of.
    index.documents = vec![document("alone", "1.2.8.99"), document("own", &own)];
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    rail.other_open = true;
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let find = |entry: &str| card_of(&drawn.seen, &document_named(&state, entry));
    let heading = drawn.seen.words.iter();
    let unplaced = heading
        .into_iter()
        .find(|(word, _)| word.starts_with("Documents of"));
    let (_, unplaced) = unplaced.expect("a heading over them");
    assert!(
        find("own").bottom() <= unplaced.top(),
        "its own study's first"
    );
    assert!(
        unplaced.bottom() <= find("alone").top(),
        "under the heading"
    );
}
