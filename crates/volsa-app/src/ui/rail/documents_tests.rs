//! Documents in the rail: listed with the other files, counted, opened by a click, and kept
//! with the rows from one frame to the next.

use volsa_dicom::testkit::{DocumentOptions, encapsulated_pdf};
use volsa_dicom::{DocumentKind, IndexedDocument};

use super::card_tests::right_click;
use super::document_card::{self, NO_TITLE, OPEN};
use super::tests::{click, pass, top_of};
use super::*;
use crate::action::{Action, DocumentAction};
use crate::documents::Mask;
use crate::reducer::reduce;
use crate::testing::documents::{
    NAMED, NOTE, PDF, REPORT, document_files, documents_alone, page, scans_and_a_named_report,
    scans_and_documents,
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

/// What a screen reader calls the card of a document, with nothing left out of it.
fn card_name(document: &IndexedDocument) -> String {
    document_card::card_name(document, &Mask::off())
}

fn card_of(seen: &Seen, document: &IndexedDocument) -> Rect {
    let name = card_name(document);
    let found = seen.place_starting(&name);
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
        let name = card_name(document);
        assert!(
            closed.seen.place_starting(&name).is_none(),
            "{name}: closed"
        );
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
    assert_eq!(card_name(&documents[0]), "Report, Made-up report");
    assert_eq!(card_name(&documents[1]), "Report (PDF), Made-up page");
    let note = card_name(&documents[2]);
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
    assert_eq!(
        clicked.actions,
        [Action::Document(DocumentAction::Open(REPORT.to_owned()))]
    );
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
        (
            OPEN,
            (
                vec![Action::Document(DocumentAction::Open(NOTE.to_owned()))],
                vec![],
            ),
        ),
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
    let shown = drawn.seen.place_starting(&card_name(&first));
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
    reduce(
        &mut state,
        Action::Document(DocumentAction::Open(PDF.to_owned())),
    );
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    assert!(rail.other_open, "opened as the document comes to be shown");
    let pdf = document_named(&state, PDF);
    assert!(drawn.seen.place_starting(&card_name(&pdf)).is_some());
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
    reduce(
        &mut state,
        Action::Document(DocumentAction::Open(REPORT.to_owned())),
    );
    assert_eq!(drawn(&state, &mut rail), 2);
    reduce(&mut state, Action::Document(DocumentAction::Close));
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
        patient_name: None,
        patient_id: None,
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

#[test]
fn a_title_that_holds_the_patients_name_is_hidden_on_the_card_in_its_hover_and_to_a_screen_reader()
{
    let mut state = AppState {
        index: Some(scans_and_a_named_report()),
        ..AppState::default()
    };
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    rail.other_open = true;
    let named = document_named(&state, NAMED);
    let mask = Mask::for_document(&state, &named);
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    // The card's name for a screen reader, its button's, its words and its hover text.
    let name = document_card::card_name(&named, &mask);
    assert_eq!(name, "Report, Report on [hidden]");
    assert!(
        drawn.seen.place_starting(&name).is_some(),
        "{:?}",
        drawn.seen.names
    );
    let said: Vec<String> = drawn
        .seen
        .words
        .iter()
        .map(|(word, _)| word.to_lowercase())
        .chain(drawn.seen.names.iter().map(|(name, _)| name.to_lowercase()))
        .collect();
    assert!(
        said.iter().all(|text| !text.contains("phantom")),
        "{said:?}"
    );
    let hover = document_card::hover_of(&named, &mask);
    assert!(!hover.to_lowercase().contains("phantom"), "{hover}");
    assert!(hover.contains("Report on [hidden]"), "{hover}");
    // With the details on, as written.
    state.show_patient = true;
    let mask = Mask::for_document(&state, &named);
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let name = document_card::card_name(&named, &mask);
    assert_eq!(name, "Report, Report on Phantom^Test");
    assert!(
        drawn.seen.place_starting(&name).is_some(),
        "{:?}",
        drawn.seen.names
    );
    assert!(document_card::hover_of(&named, &mask).contains("Report on Phantom^Test"));
}

#[test]
fn the_title_of_a_pdf_in_an_export_of_documents_alone_hides_the_name_its_file_gives() {
    // A PDF titled with the made-up patient's name, in an export with no scan at all.
    let titled = DocumentOptions {
        title: "Letter about Test Phantom".to_owned(),
        ..DocumentOptions::default()
    };
    let files = vec![("DOCS/L0001".to_owned(), encapsulated_pdf(&titled, &page()))];
    let mut state = AppState {
        index: Some(indexed(files)),
        ..AppState::default()
    };
    assert!(
        state
            .index
            .as_ref()
            .is_some_and(|index| index.studies.is_empty())
    );
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    rail.other_open = true;
    let named = document_named(&state, "DOCS/L0001");
    let mask = Mask::for_document(&state, &named);
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let name = document_card::card_name(&named, &mask);
    assert_eq!(name, "Report (PDF), Letter about [hidden]");
    assert!(
        drawn.seen.place_starting(&name).is_some(),
        "{:?}",
        drawn.seen.names
    );
    let said: Vec<String> = drawn
        .seen
        .words
        .iter()
        .map(|(word, _)| word.to_lowercase())
        .chain(drawn.seen.names.iter().map(|(name, _)| name.to_lowercase()))
        .collect();
    assert!(
        said.iter().all(|text| !text.contains("phantom")),
        "{said:?}"
    );
    let hover = document_card::hover_of(&named, &mask);
    assert!(!hover.to_lowercase().contains("phantom"), "{hover}");
    // With the details on, as written.
    state.show_patient = true;
    let shown = Mask::for_document(&state, &named);
    assert_eq!(
        document_card::card_name(&named, &shown),
        "Report (PDF), Letter about Test Phantom"
    );
}
