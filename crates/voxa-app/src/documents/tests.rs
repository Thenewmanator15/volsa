use voxa_core::{StudyId, StudyInfo};

use super::*;

fn study(uid: &str, date: &str) -> StudyInfo {
    StudyInfo {
        id: StudyId(uid.to_owned()),
        date: Some(date.to_owned()),
        description: String::new(),
        patient_name: None,
        patient_id: None,
        series: Vec::new(),
    }
}

fn document(entry: &str, study_uid: &str, kind: DocumentKind) -> IndexedDocument {
    IndexedDocument {
        entry: entry.to_owned(),
        study_uid: study_uid.to_owned(),
        kind,
        title: String::new(),
        date: None,
        patient_name: None,
        patient_id: None,
    }
}

/// An export of an older study, a newer one, and documents of both and of a third study that
/// holds no images, as the index lists them: in the order of its studies, then the third.
fn export() -> Index {
    let documents = vec![
        document("old-1", "1.2.8.0", DocumentKind::Report),
        document("old-2", "1.2.8.0", DocumentKind::Pdf),
        document("new-1", "1.2.8.1", DocumentKind::Report),
        document("lone-1", "1.2.8.9", DocumentKind::Report),
    ];
    Index {
        studies: vec![study("1.2.8.0", "20190101"), study("1.2.8.1", "20200101")],
        documents,
        ..Index::default()
    }
}

fn entries(index: &Index, places: &[usize]) -> Vec<String> {
    let all = &index.documents;
    places.iter().map(|at| all[*at].entry.clone()).collect()
}

#[test]
fn a_studys_documents_are_those_that_say_they_belong_to_it() {
    let index = export();
    assert_eq!(entries(&index, &of_study(&index, 0)), ["old-1", "old-2"]);
    assert_eq!(entries(&index, &of_study(&index, 1)), ["new-1"]);
    assert_eq!(
        of_study(&index, 2),
        Vec::<usize>::new(),
        "there is no third"
    );
}

#[test]
fn the_documents_of_studies_with_no_images_are_apart() {
    let index = export();
    assert_eq!(entries(&index, &unplaced(&index)), ["lone-1"]);
    let alone = Index {
        documents: index.documents,
        ..Index::default()
    };
    assert_eq!(unplaced(&alone).len(), 4, "with no studies, all are");
}

#[test]
fn the_first_document_is_the_first_the_rail_lists() {
    // The rail lists the newer study first.
    let index = export();
    assert_eq!(first(&index).map(|doc| doc.entry.as_str()), Some("new-1"));
    // Studies with no images come last.
    let alone = Index {
        studies: Vec::new(),
        documents: index.documents[3..].to_vec(),
        ..Index::default()
    };
    assert_eq!(first(&alone).map(|doc| doc.entry.as_str()), Some("lone-1"));
    assert_eq!(first(&Index::default()), None);
}

#[test]
fn a_kind_is_said_in_words_without_anything_that_turns_or_hides_text() {
    assert_eq!(kind_words(&DocumentKind::Report), "Report");
    assert_eq!(kind_words(&DocumentKind::Pdf), "Report (PDF)");
    let note = DocumentKind::Other("Key-image note".to_owned());
    assert_eq!(kind_words(&note), "Key-image note");
    let turned = DocumentKind::Other("RT\u{202E} Plan\u{200B}\n".to_owned());
    let said = kind_words(&turned);
    assert!(said.starts_with("RT") && said.contains("Plan"), "{said:?}");
    assert!(
        !said
            .chars()
            .any(|c| c.is_control() || c == '\u{202E}' || c == '\u{200B}')
    );
}
