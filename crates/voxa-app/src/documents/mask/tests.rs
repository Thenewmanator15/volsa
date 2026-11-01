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
        patient_name: None,
        patient_id: None,
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
    // A short identifier is not looked for at all: what it would change is the report's words.
    let short = Index {
        studies: vec![study("1.2.8.0", None, Some("1"))],
        ..Index::default()
    };
    let mask = Mask::of(&short, &document("1.2.8.0"), false);
    assert_eq!(
        mask.apply("Width: 12 mm, 1 of 21").as_ref(),
        "Width: 12 mm, 1 of 21"
    );
}

/// The mask of a study whose patient is `name` and `id`.
fn mask_of(name: Option<&str>, id: Option<&str>) -> Mask {
    let named = Index {
        studies: vec![study("1.2.8.0", name, id)],
        ..Index::default()
    };
    Mask::of(&named, &document("1.2.8.0"), false)
}

#[test]
fn a_short_or_numeric_identifier_never_changes_a_number() {
    // Too short to be looked for, and the numbers around it are left alone.
    let short = mask_of(None, Some("10"));
    for text in [
        "10.5 mm",
        "0.10",
        "1,10",
        "Width: 10 mm",
        "a size of 0.10 cm",
    ] {
        assert_eq!(short.apply(text).as_ref(), text);
    }
    // Long enough to be looked for: found as a word of its own, never taken from a number.
    let long = mask_of(None, Some("12345"));
    assert_eq!(long.apply("MRN 12345.").as_ref(), "MRN [hidden].");
    assert_eq!(
        long.apply("MRN: 12345, seen").as_ref(),
        "MRN: [hidden], seen"
    );
    for text in ["12345.6 mm", "0.12345", "3,12345", "12345,6", "1.2345"] {
        assert_eq!(long.apply(text).as_ref(), text);
    }
    // A number that merely ends a sentence is the identifier all the same.
    assert_eq!(
        long.apply("Seen 12345. Next 2.").as_ref(),
        "Seen [hidden]. Next 2."
    );
}

#[test]
fn a_name_of_under_three_letters_is_not_looked_for_but_one_of_three_is() {
    let two = mask_of(Some("Wu"), None);
    assert_eq!(two.apply("Seen by Wu").as_ref(), "Seen by Wu");
    let three = mask_of(Some("Lee"), None);
    assert_eq!(three.apply("Seen by Lee").as_ref(), "Seen by [hidden]");
    // A name of two short parts is long enough written out whole: "Li^Wu" and "Li Wu".
    let parts = mask_of(Some("Li^Wu"), None);
    assert_eq!(
        parts.apply("Li Wu and Wu Li").as_ref(),
        "[hidden] and [hidden]"
    );
    assert_eq!(parts.apply("Wu").as_ref(), "Wu");
}

#[test]
fn a_needle_of_punctuation_alone_is_not_looked_for() {
    let dashes = mask_of(Some("^"), Some("-"));
    for text in ["T1-weighted, -5 mm", "a^b", "^^"] {
        assert_eq!(dashes.apply(text).as_ref(), text);
    }
    let marks = mask_of(Some("^^"), Some("--- "));
    assert_eq!(marks.apply("T1--- weighted").as_ref(), "T1--- weighted");
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

/// A document of `study_uid` whose own file names the patient `name` and `id`.
fn document_naming(study_uid: &str, name: &str, id: &str) -> IndexedDocument {
    IndexedDocument {
        patient_name: Some(name.to_owned()),
        patient_id: Some(id.to_owned()),
        ..document(study_uid)
    }
}

#[test]
fn the_name_a_documents_own_file_gives_is_hidden_when_the_export_names_nobody() {
    // An export of documents alone: no study, so no scan to say whose they are.
    let alone = Index::default();
    let report = document_naming("1.2.8.0", "Phantom^Test", "PHANTOM-001");
    let mask = Mask::of(&alone, &report, false);
    assert_ne!(mask, Mask::off());
    assert_eq!(
        mask.apply("Reviewed for Test Phantom (PHANTOM-001), Phantom^Test")
            .as_ref(),
        "Reviewed for [hidden] ([hidden]), [hidden]"
    );
    // With the details shown, as written.
    let shown = Mask::of(&alone, &report, true);
    assert_eq!(shown, Mask::off());
}

#[test]
fn the_documents_own_name_is_hidden_beside_the_name_of_the_study_it_is_found_in() {
    // The document's file gives another person than the scans of its study do: both go.
    let other = document_naming("1.2.8.0", "Madeup^Visitor", "VISITOR-42");
    let mask = Mask::of(&export(), &other, false);
    assert_eq!(
        mask.apply("Phantom Test, Visitor Madeup and VISITOR-42")
            .as_ref(),
        "[hidden], [hidden] and [hidden]"
    );
    // And one of a study the scans do not hold: everyone's, and its own.
    let unplaced = document_naming("1.2.8.99", "Madeup^Visitor", "VISITOR-42");
    let mask = Mask::of(&export(), &unplaced, false);
    assert_eq!(
        mask.apply("Phantom Test, MADEUP-7, Madeup Visitor")
            .as_ref(),
        "[hidden], [hidden], [hidden]"
    );
}

#[test]
fn a_name_broken_over_two_lines_of_a_text_is_still_found() {
    // A line break in the middle of a name is as good as a space: the text keeps its breaks to
    // be drawn, and the name is no less hidden for being split.
    let text = "Reviewed for Test\nPhantom today; PHANTOM\nTEST again";
    assert_eq!(hidden(text), "Reviewed for [hidden] today; [hidden] again");
    // And a break that is not inside a name is left where it is.
    assert_eq!(hidden("Width: 12 mm\nClosing"), "Width: 12 mm\nClosing");
}

#[test]
fn a_mask_of_everyone_in_the_export_works_on_any_string_with_no_document() {
    // For the words of anything that may name a patient but is no report: a file's name or path.
    let everyone = Mask::of_everyone(&export(), false);
    assert_eq!(
        everyone
            .apply("DICOM/Phantom Test/MADEUP-7/I0001.dcm")
            .as_ref(),
        "DICOM/[hidden]/[hidden]/I0001.dcm"
    );
    assert_eq!(
        everyone.apply("DICOM/S1/I0001.dcm").as_ref(),
        "DICOM/S1/I0001.dcm"
    );
    // With the details shown, or with no export open, everything is as written.
    assert_eq!(Mask::of_everyone(&export(), true), Mask::off());
    let state = AppState {
        index: Some(export()),
        ..AppState::default()
    };
    assert_eq!(Mask::for_export(&state).apply("MADEUP-7").as_ref(), HIDDEN);
    let closed = AppState::default();
    assert_eq!(
        Mask::for_export(&closed).apply("MADEUP-7").as_ref(),
        "MADEUP-7"
    );
}
