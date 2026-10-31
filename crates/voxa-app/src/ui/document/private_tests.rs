//! What a document leaves out while the patient's details are hidden, drawn in the window: a
//! name written in a report or its title is hidden while the details are off and shown as
//! written while they are on.

use voxa_dicom::read_report;

use super::tests::{opened_with, pass_with, words_of};
use super::*;
use crate::documents::mask::NAME_HIDDEN;
use crate::testing::documents::{NAMED, NAMED_TITLE, named_report_file, scans_and_a_named_report};
use crate::ui::seen::{self, Seen};

/// A state that has read the made-up export and the report that names its patient, and shows
/// that report; the patient's details are hidden, as they are in every run.
fn naming_the_patient() -> AppState {
    let report = read_report(&named_report_file().1).expect("a made-up report reads");
    opened_with(scans_and_a_named_report(), NAMED, Some(Ok(report)))
}

/// Everything a pass drew or told a screen reader, in lower case.
fn everything(seen: &Seen) -> Vec<String> {
    let words = seen.words.iter().map(|(word, _)| word.to_lowercase());
    let names = seen.names.iter().map(|(name, _)| name.to_lowercase());
    words.chain(names).collect()
}

fn names_the_patient(seen: &Seen) -> bool {
    let said = everything(seen);
    said.iter().any(|text| text.contains("phantom"))
}

#[test]
fn a_name_in_a_report_is_hidden_while_details_are_off_and_shown_as_written_while_on() {
    let ctx = seen::context(1.0);
    let (mut kept, mut state) = (Kept::default(), naming_the_patient());
    let hidden = pass_with(&ctx, &state, &mut kept, Vec::new());
    let words = words_of(&hidden.seen);
    // An item that holds a name keeps its own name and loses the value.
    let by = format!("Seen by: {NAME_HIDDEN}");
    assert!(words.contains(&by.as_str()), "{words:?}");
    // The name written out in a sentence, the other way round or in small letters, and the
    // identifier, are each replaced; the rest is as written.
    assert!(
        words.contains(&"Note: Reviewed for [hidden] ([hidden]) today."),
        "{words:?}"
    );
    assert!(words.contains(&"Again: [hidden], once more"), "{words:?}");
    assert!(words.contains(&"Width: 12 mm"), "{words:?}");
    assert!(
        !names_the_patient(&hidden.seen),
        "{:?}",
        everything(&hidden.seen)
    );
    assert!(
        !everything(&hidden.seen)
            .iter()
            .any(|text| text.contains("001"))
    );
    // With the details on, with what is kept from before, everything is as written.
    state.show_patient = true;
    let shown = pass_with(&ctx, &state, &mut kept, Vec::new());
    let words = words_of(&shown.seen);
    assert!(words.contains(&"Seen by: Phantom^Test"), "{words:?}");
    assert!(words.contains(&"Note: Reviewed for Test Phantom (PHANTOM-001) today."));
    assert!(words.contains(&"Again: phantom test, once more"));
    assert!(!words.iter().any(|word| word.contains("[hidden]")));
    // And turned off again, hidden again.
    state.show_patient = false;
    let again = pass_with(&ctx, &state, &mut kept, Vec::new());
    assert!(!names_the_patient(&again.seen));
}

#[test]
fn a_title_that_holds_the_name_is_hidden_in_the_header_while_details_are_off() {
    let ctx = seen::context(1.0);
    let (mut kept, mut state) = (Kept::default(), naming_the_patient());
    let hidden = pass_with(&ctx, &state, &mut kept, Vec::new());
    let words = words_of(&hidden.seen);
    assert!(
        words.contains(&"Report on [hidden] · 2 January 2020"),
        "{words:?}"
    );
    state.show_patient = true;
    let shown = pass_with(&ctx, &state, &mut kept, Vec::new());
    let said = format!("{NAMED_TITLE} · 2 January 2020");
    assert!(words_of(&shown.seen).contains(&said.as_str()));
}

#[test]
fn a_report_of_a_study_whose_patient_the_export_does_not_name_still_hides_the_items_that_hold_names()
 {
    // Nobody to look for in the words, but an item that holds a name is hidden all the same.
    let mut state = naming_the_patient();
    if let Some(index) = state.index.as_mut() {
        index.studies.clear();
    }
    let ctx = seen::context(1.0);
    let drawn = pass_with(&ctx, &state, &mut Kept::default(), Vec::new());
    let words = words_of(&drawn.seen);
    let by = format!("Seen by: {NAME_HIDDEN}");
    assert!(words.contains(&by.as_str()), "{words:?}");
}

#[test]
fn a_report_in_an_export_of_documents_alone_hides_the_name_its_file_gives() {
    // No scan, so no study: the only place the export says whose report this is, is the report.
    let mut state = naming_the_patient();
    if let Some(index) = state.index.as_mut() {
        index.studies.clear();
    }
    let ctx = seen::context(1.0);
    let (mut kept, mut shown) = (Kept::default(), state);
    let hidden = pass_with(&ctx, &shown, &mut kept, Vec::new());
    let words = words_of(&hidden.seen);
    assert!(
        words.contains(&"Note: Reviewed for [hidden] ([hidden]) today."),
        "{words:?}"
    );
    assert!(words.contains(&"Again: [hidden], once more"), "{words:?}");
    assert!(
        words.contains(&"Report on [hidden] · 2 January 2020"),
        "the title in the header: {words:?}"
    );
    assert!(
        !names_the_patient(&hidden.seen),
        "{:?}",
        everything(&hidden.seen)
    );
    // With the details on, as written.
    shown.show_patient = true;
    let written = pass_with(&ctx, &shown, &mut kept, Vec::new());
    let words = words_of(&written.seen);
    assert!(words.contains(&"Note: Reviewed for Test Phantom (PHANTOM-001) today."));
}
