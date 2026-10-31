//! Which files of an export are documents, in what order, and the PDF inside one.

use super::*;
use crate::index::{Index, index};
use crate::testkit::{
    DocumentOptions, MemorySource, ReportItem, SliceOptions, classic_slice, encapsulated_pdf,
    structured_report,
};

const PRESENTATION_STATE: &str = "1.2.840.10008.5.1.4.1.1.11.1";

/// A report of a few lines, made up.
pub(super) fn a_report(opts: &DocumentOptions) -> Vec<u8> {
    let content = [
        ReportItem::container(
            "Part A",
            vec![
                ReportItem::text("Comment", "A made-up remark."),
                ReportItem::number("Width", "12", "mm"),
            ],
        ),
        ReportItem::text("Closing", "That is all."),
    ];
    structured_report(opts, &content)
}

/// A made-up page of a PDF, of odd length.
fn a_page() -> Vec<u8> {
    let mut page = b"%PDF-1.4\nA made-up page\n".to_vec();
    page.extend_from_slice(&[0, 1, 2, 250, 255]);
    page.extend_from_slice(b"\n%%EOF\n");
    page.push(b'.');
    assert_eq!(page.len() % 2, 1, "the test needs a page of odd length");
    page
}

/// `count` slices of one series, named under `IMAGES/`.
fn slices(count: u32) -> Vec<(String, Vec<u8>)> {
    (0..count)
        .map(|at| {
            let opts = SliceOptions {
                instance_number: i32::try_from(at + 1).expect("small"),
                position: Some([-100.0, -120.0, f64::from(at) * 5.0]),
                ..SliceOptions::default()
            };
            (format!("IMAGES/I{at:04}"), classic_slice(&opts))
        })
        .collect()
}

fn indexed(files: Vec<(String, Vec<u8>)>) -> Index {
    index(&mut MemorySource::new(files), &mut |_| {}).expect("indexing does not fail")
}

fn named(entry: &str, bytes: Vec<u8>) -> (String, Vec<u8>) {
    (entry.to_owned(), bytes)
}

#[test]
fn a_report_is_listed_as_a_document_not_skipped() {
    let opts = DocumentOptions {
        title: "Made-up report".to_owned(),
        ..DocumentOptions::default()
    };
    let mut files = slices(12);
    files.push(named("REPORT/R0001", a_report(&opts)));
    let found = indexed(files);
    assert_eq!(
        found.documents,
        vec![IndexedDocument {
            entry: "REPORT/R0001".to_owned(),
            study_uid: opts.study_uid.clone(),
            kind: DocumentKind::Report,
            title: "Made-up report".to_owned(),
            date: Some("20200102".to_owned()),
        }]
    );
    assert_eq!(found.skipped, Vec::<String>::new());
}

#[test]
fn a_pdf_is_listed_as_a_document() {
    let opts = DocumentOptions {
        title: "A made-up page".to_owned(),
        ..DocumentOptions::default()
    };
    let found = indexed(vec![named("DOCS/D1", encapsulated_pdf(&opts, &a_page()))]);
    assert_eq!(
        found.documents,
        vec![IndexedDocument {
            entry: "DOCS/D1".to_owned(),
            study_uid: opts.study_uid,
            kind: DocumentKind::Pdf,
            title: "A made-up page".to_owned(),
            date: Some("20200102".to_owned()),
        }]
    );
    assert_eq!(found.skipped, Vec::<String>::new());
    assert_eq!(found.series, Vec::new());
}

#[test]
fn images_are_still_indexed_as_before() {
    let plain = indexed(slices(12));
    assert_eq!(plain.documents, Vec::new());

    let mut files = slices(12);
    files.push(named("REPORT/R0001", a_report(&DocumentOptions::default())));
    files.push(named(
        "DOCS/D1",
        encapsulated_pdf(&DocumentOptions::default(), &a_page()),
    ));
    files.push(named("notes/readme.txt", b"hello".to_vec()));
    let with_documents = indexed(files);
    assert_eq!(with_documents.series, plain.series);
    assert_eq!(with_documents.studies, plain.studies);
    assert_eq!(with_documents.documents.len(), 2);
    assert_eq!(with_documents.skipped, vec!["notes/readme.txt".to_owned()]);
}

#[test]
fn other_documents_are_named_in_words_where_the_class_is_known() {
    let class = |uid: &str| DocumentOptions {
        sop_class: Some(uid.to_owned()),
        ..DocumentOptions::default()
    };
    let files = vec![
        named("a", structured_report(&class(PRESENTATION_STATE), &[])),
        named(
            "b",
            structured_report(&class("1.2.826.0.1.3680043.9.9999.5"), &[]),
        ),
    ];
    let kinds: Vec<DocumentKind> = indexed(files)
        .documents
        .into_iter()
        .map(|d| d.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            DocumentKind::Other("Grayscale Softcopy Presentation State".to_owned()),
            DocumentKind::Other("1.2.826.0.1.3680043.9.9999.5".to_owned()),
        ]
    );
}

#[test]
fn a_document_is_titled_by_its_own_title_then_its_series_then_nothing() {
    let with_words = |title: &str, series: &str| DocumentOptions {
        title: title.to_owned(),
        series_description: series.to_owned(),
        ..DocumentOptions::default()
    };
    let files = vec![
        named("a", a_report(&with_words("Report title", "Series words"))),
        named("b", a_report(&with_words("", "Series words"))),
        named("c", a_report(&with_words("", ""))),
        named(
            "d",
            encapsulated_pdf(&with_words("", "Series words"), &a_page()),
        ),
    ];
    let titles: Vec<String> = indexed(files)
        .documents
        .into_iter()
        .map(|d| d.title)
        .collect();
    assert_eq!(titles, ["Report title", "Series words", "", "Series words"]);
}

#[test]
fn a_document_is_dated_by_when_it_was_made_then_by_its_study() {
    let with_dates = |study: Option<&str>, content: Option<&str>| DocumentOptions {
        study_date: study.map(str::to_owned),
        content_date: content.map(str::to_owned),
        ..DocumentOptions::default()
    };
    let files = vec![
        named(
            "a",
            a_report(&with_dates(Some("20200102"), Some("20200105"))),
        ),
        named("b", a_report(&with_dates(Some("20200102"), None))),
        named("c", a_report(&with_dates(None, None))),
    ];
    let dates: Vec<Option<String>> = indexed(files)
        .documents
        .into_iter()
        .map(|d| d.date)
        .collect();
    assert_eq!(
        dates,
        [
            Some("20200105".to_owned()),
            Some("20200102".to_owned()),
            None
        ]
    );
}

#[test]
fn documents_come_in_study_order_then_by_entry_name() {
    let in_study = |study: &str, date: Option<&str>| DocumentOptions {
        study_uid: study.to_owned(),
        study_date: date.map(str::to_owned),
        ..DocumentOptions::default()
    };
    let later = SliceOptions {
        study_uid: "1.2.826.0.1.3680043.9.9999.2".to_owned(),
        series_uid: "1.2.826.0.1.3680043.9.9999.2.1".to_owned(),
        study_date: Some("20210101".to_owned()),
        ..SliceOptions::default()
    };
    let mut files = slices(2);
    files.push(named("LATER/I1", classic_slice(&later)));
    // Study 1 is dated 2020 and study 2 is dated 2021. The studies of the last three hold no
    // images, so they come last, by their identifiers.
    let wanted = [
        ("A/z", "1.2.826.0.1.3680043.9.9999.1", Some("20200102")),
        ("B/a", "1.2.826.0.1.3680043.9.9999.1", Some("20200102")),
        ("A/y", "1.2.826.0.1.3680043.9.9999.2", Some("20210101")),
        ("C/zero", "1.2.826.0.1.3680043.9.9999.0", None),
        ("C/three", "1.2.826.0.1.3680043.9.9999.3", Some("20190101")),
        ("C/seven", "1.2.826.0.1.3680043.9.9999.7", Some("20220101")),
    ];
    // Given in an order that is neither the expected one nor the reverse of it.
    for at in [3, 0, 5, 2, 4, 1] {
        let (entry, study, date) = wanted[at];
        files.push(named(entry, a_report(&in_study(study, date))));
    }
    let entries: Vec<String> = indexed(files)
        .documents
        .into_iter()
        .map(|d| d.entry)
        .collect();
    assert_eq!(
        entries,
        ["A/z", "B/a", "A/y", "C/zero", "C/three", "C/seven"]
    );
}

#[test]
fn a_dicom_file_that_names_no_study_is_skipped_like_any_other_file() {
    let nameless = DocumentOptions {
        study_uid: String::new(),
        ..DocumentOptions::default()
    };
    let found = indexed(vec![named("odd", a_report(&nameless))]);
    assert_eq!(found.documents, Vec::new());
    assert_eq!(found.skipped, vec!["odd".to_owned()]);
}

#[test]
fn an_image_with_a_missing_identifier_is_not_taken_for_a_document() {
    let nameless = SliceOptions {
        series_uid: String::new(),
        ..SliceOptions::default()
    };
    let found = indexed(vec![named("broken", classic_slice(&nameless))]);
    assert_eq!(found.documents, Vec::new());
    assert_eq!(found.skipped, vec!["broken".to_owned()]);
}

#[test]
fn the_pdf_inside_is_given_back_whole() {
    let page = a_page();
    let even = [page.clone(), vec![b'!']].concat();
    for original in [page, even] {
        let file = encapsulated_pdf(&DocumentOptions::default(), &original);
        assert_eq!(pdf_bytes(&file).expect("has a pdf"), original);
    }
}

#[test]
fn what_holds_no_pdf_is_an_error_not_a_panic() {
    let file = encapsulated_pdf(&DocumentOptions::default(), &a_page());
    assert!(matches!(
        pdf_bytes(b"%PDF-1.4 not in a DICOM file"),
        Err(VoxaError::NotDicom)
    ));
    assert!(matches!(pdf_bytes(&[]), Err(VoxaError::NotDicom)));
    for cut in [140, file.len() / 2, file.len() - 1] {
        let result = pdf_bytes(&file[..cut]);
        assert!(result.is_err(), "{cut} bytes of {} gave a pdf", file.len());
    }
    let report = a_report(&DocumentOptions::default());
    assert!(pdf_bytes(&report).is_err(), "a report holds no pdf");
    let image = classic_slice(&SliceOptions::default());
    assert!(pdf_bytes(&image).is_err(), "an image holds no pdf");
}
