//! What is left out of a document's words while the patient's details are hidden.

use voxa_core::{StudyId, StudyInfo};
use voxa_dicom::DocumentKind;

use super::*;

fn study(uid: &str, name: Option<&str>, id: Option<&str>) -> StudyInfo {
    StudyInfo {
        id: StudyId(uid.to_owned()),
        date: None,
        description: String::new(),
        patient_name: name.map(str::to_owned),
        patient_id: id.map(str::to_owned),
        series: Vec::new(),
    }
}

fn document(study_uid: &str) -> IndexedDocument {
    IndexedDocument {
        entry: "DOCS/A".to_owned(),
        study_uid: study_uid.to_owned(),
        kind: DocumentKind::Report,
        title: String::new(),
        date: None,
    }
}

/// An export of the studies of two made-up people.
fn export() -> Index {
    Index {
        studies: vec![
            study("1.2.8.0", Some("Phantom^Test"), Some("PHANTOM-001")),
            study("1.2.8.1", Some("Madeup^Other^Third"), Some("MADEUP-7")),
        ],
        ..Index::default()
    }
}

fn hidden(text: &str) -> String {
    let mask = Mask::of(&export(), &document("1.2.8.0"), false);
    mask.apply(text).into_owned()
}

#[test]
fn the_patients_name_and_identifier_are_replaced_wherever_they_are_written_out() {
    assert_eq!(hidden("Phantom^Test"), HIDDEN, "as the file writes it");
    assert_eq!(hidden("Seen: Phantom Test."), "Seen: [hidden].");
    assert_eq!(
        hidden("For Test Phantom"),
        "For [hidden]",
        "the other order"
    );
    assert_eq!(
        hidden("PHANTOM TEST and phantom test"),
        "[hidden] and [hidden]"
    );
    assert_eq!(
        hidden("ID PHANTOM-001, again phantom-001"),
        "ID [hidden], again [hidden]"
    );
    assert_eq!(hidden("PHANTOM-001 Phantom Test"), "[hidden] [hidden]");
    // A name of three parts: in order and the other way round.
    let mask = Mask::of(&export(), &document("1.2.8.1"), false);
    assert_eq!(mask.apply("Madeup Other Third").as_ref(), HIDDEN);
    assert_eq!(mask.apply("Third Other Madeup").as_ref(), HIDDEN);
}

#[test]
fn only_the_patient_of_the_study_the_document_belongs_to_is_left_out() {
    // The other person's name is another person's, and is not this document's patient.
    assert_eq!(hidden("Madeup Other Third"), "Madeup Other Third");
    assert_eq!(hidden("MADEUP-7"), "MADEUP-7");
}

#[test]
fn a_document_of_a_study_with_no_scans_leaves_out_everyone_the_export_holds() {
    let mask = Mask::of(&export(), &document("1.2.8.99"), false);
    assert_eq!(
        mask.apply("Phantom Test, MADEUP-7").as_ref(),
        "[hidden], [hidden]"
    );
}

#[test]
fn a_name_is_not_found_inside_a_longer_word_nor_are_numbers_taken_apart() {
    assert_eq!(hidden("Phantomatic Testing"), "Phantomatic Testing");
    assert_eq!(
        hidden("xPHANTOM-001 PHANTOM-0012"),
        "xPHANTOM-001 PHANTOM-0012"
    );
    assert_eq!(hidden("A width of 12 mm"), "A width of 12 mm");
    // A short identifier is only found whole, and shows where it was.
    let short = Index {
        studies: vec![study("1.2.8.0", None, Some("1"))],
        ..Index::default()
    };
    let mask = Mask::of(&short, &document("1.2.8.0"), false);
    assert_eq!(
        mask.apply("Width: 12 mm, 1 of 21").as_ref(),
        "Width: 12 mm, [hidden] of 21"
    );
}

#[test]
fn letters_are_matched_whatever_their_case_in_any_script() {
    let named = Index {
        studies: vec![study("1.2.8.0", Some("Müller^Åsa"), None)],
        ..Index::default()
    };
    let mask = Mask::of(&named, &document("1.2.8.0"), false);
    assert_eq!(
        mask.apply("MÜLLER ÅSA and åsa müller").as_ref(),
        "[hidden] and [hidden]"
    );
}

#[test]
fn words_with_nothing_to_hide_are_not_copied() {
    let mask = Mask::of(&export(), &document("1.2.8.0"), false);
    assert!(matches!(mask.apply("Width: 12 mm"), Cow::Borrowed(_)));
    assert!(matches!(
        Mask::off().apply("Phantom Test"),
        Cow::Borrowed(_)
    ));
}

#[test]
fn with_the_details_shown_everything_is_as_written() {
    let mask = Mask::of(&export(), &document("1.2.8.0"), true);
    assert_eq!(mask, Mask::off());
    assert_eq!(
        mask.apply("Phantom Test PHANTOM-001").as_ref(),
        "Phantom Test PHANTOM-001"
    );
    assert_eq!(
        mask.words("Seen by: Phantom^Test", true).as_ref(),
        "Seen by: Phantom^Test"
    );
}

#[test]
fn an_item_that_holds_a_name_keeps_its_own_name_and_loses_the_value() {
    let mask = Mask::of(&export(), &document("1.2.8.0"), false);
    let said = |text: &str| mask.words(text, true).into_owned();
    assert_eq!(
        said("Seen by: Someone^Else"),
        format!("Seen by: {NAME_HIDDEN}")
    );
    assert_eq!(said("Name: Someone^Else"), format!("Name: {NAME_HIDDEN}"));
    // A value with a colon in it is all the value, and a concept with one is cut where it is
    // told from the value: the value never shows.
    assert_eq!(
        said("Seen by: Else: Someone"),
        format!("Seen by: {NAME_HIDDEN}")
    );
    assert_eq!(said("Seen: by: Someone"), format!("Seen: {NAME_HIDDEN}"));
    assert_eq!(said("Someone^Else"), NAME_HIDDEN);
    // With no value there is nothing to hide, and the line says so.
    assert_eq!(said("Seen by:"), "Seen by:");
    // Any other line is left as it is but for the patient's own name.
    assert_eq!(
        mask.words("Note: Phantom Test", false).as_ref(),
        "Note: [hidden]"
    );
    assert_eq!(
        NAME_HIDDEN,
        "Name hidden (turn on Show patient details to see it)"
    );
}

#[test]
fn a_study_that_gives_no_name_or_identifier_leaves_nothing_to_find_but_names_are_still_hidden() {
    let nameless = Index {
        studies: vec![study("1.2.8.0", Some("  "), None)],
        ..Index::default()
    };
    let mask = Mask::of(&nameless, &document("1.2.8.0"), false);
    assert_ne!(mask, Mask::off());
    assert_eq!(mask.apply("Phantom Test").as_ref(), "Phantom Test");
    assert_eq!(
        mask.words("A: Anyone", true).as_ref(),
        format!("A: {NAME_HIDDEN}")
    );
}
