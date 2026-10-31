//! Files in an export that hold a document rather than an image: structured reports and PDFs.

use std::io::Cursor;

use dicom_core::dictionary::UidDictionary;
use dicom_dictionary_std::{StandardSopClassDictionary, tags, uids};
use dicom_object::{DefaultDicomObject, InMemDicomObject, OpenFileOptions};
use voxa_core::{StudyInfo, VoxaError};

use crate::decode::{guarded, open};
use crate::header::{first_item, text};

mod report;

pub use report::{
    MAX_REPORT_CHARS, MAX_REPORT_DEPTH, MAX_REPORT_LINES, Report, ReportLine, read_report,
};

/// The start of the identifiers of the structured report storage classes.
const REPORT_CLASSES: &str = "1.2.840.10008.5.1.4.1.1.88.";

/// What a document is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentKind {
    /// A structured report, which [`read_report`] turns into lines of text.
    Report,
    /// A PDF stored in DICOM, which [`pdf_bytes`] gives back as the PDF file it was.
    Pdf,
    /// Something else that holds no image, named in words where Voxa knows the name of its
    /// SOP class and otherwise by the SOP class identifier itself.
    Other(String),
}

/// A DICOM file that holds a document rather than an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedDocument {
    /// Name of the entry in the source that holds the file.
    pub entry: String,
    /// The study the file says it belongs to.
    pub study_uid: String,
    /// What the document is.
    pub kind: DocumentKind,
    /// The document's own title, or failing that its series' description; empty when the file
    /// gives neither.
    pub title: String,
    /// The date the document was made (`YYYYMMDD`) or, failing that, the study's date.
    pub date: Option<String>,
}

/// The document a file holds, given the start of the file that indexing read: `None` when the
/// file is not DICOM, has an image, or does not say what it is and which study it belongs to.
///
/// What indexing read is the whole of a DICOM file that is not an image, so the parts of the
/// document that come last are there to be read.
pub(crate) fn of(entry: &str, start: &[u8]) -> Option<IndexedDocument> {
    guarded(|| Ok(described(entry, start))).ok().flatten()
}

fn described(entry: &str, start: &[u8]) -> Option<IndexedDocument> {
    // Not as far as the document itself, which can be large and says nothing more of it.
    let file = OpenFileOptions::new()
        .read_until(tags::ENCAPSULATED_DOCUMENT)
        .from_reader(Cursor::new(start))
        .ok()?;
    if holds_an_image(&file) {
        return None;
    }
    let kind = kind_of(&sop_class_of(&file)?);
    Some(IndexedDocument {
        entry: entry.to_owned(),
        study_uid: text(&file, tags::STUDY_INSTANCE_UID)?,
        title: title_of(&file, &kind),
        date: text(&file, tags::CONTENT_DATE).or_else(|| text(&file, tags::STUDY_DATE)),
        kind,
    })
}

/// Whether a file is shaped like an image, whatever else is wrong with it: a file that says
/// how large its picture is, or how it is stored, is left to be skipped as a damaged image,
/// not listed as a document.
fn holds_an_image(obj: &InMemDicomObject) -> bool {
    [tags::ROWS, tags::COLUMNS, tags::BITS_ALLOCATED]
        .into_iter()
        .any(|tag| obj.get(tag).is_some())
}

/// The SOP class a file says it is of: its own, or failing that the one its file header names.
fn sop_class_of(file: &DefaultDicomObject) -> Option<String> {
    text(file, tags::SOP_CLASS_UID).or_else(|| {
        let named = file
            .meta()
            .media_storage_sop_class_uid()
            .trim_end_matches(['\0', ' ']);
        (!named.is_empty()).then(|| named.to_owned())
    })
}

/// Whether a SOP class is one of a report that is read as text: a structured report, but not
/// a key-image note, which is built as one yet only points at images.
fn is_report_class(sop_class: &str) -> bool {
    sop_class.starts_with(REPORT_CLASSES)
        && sop_class != uids::KEY_OBJECT_SELECTION_DOCUMENT_STORAGE
}

fn kind_of(sop_class: &str) -> DocumentKind {
    if is_report_class(sop_class) {
        return DocumentKind::Report;
    }
    if sop_class == uids::ENCAPSULATED_PDF_STORAGE {
        return DocumentKind::Pdf;
    }
    if sop_class == uids::KEY_OBJECT_SELECTION_DOCUMENT_STORAGE {
        // The standard calls it a "Key Object Selection Document", which says little.
        return DocumentKind::Other("Key-image note".to_owned());
    }
    let known = StandardSopClassDictionary.by_uid(sop_class);
    let words = known.map(|entry| entry.name.strip_suffix(" Storage").unwrap_or(entry.name));
    DocumentKind::Other(words.unwrap_or(sop_class).to_owned())
}

/// What a document calls itself: a report's first concept name, a PDF's title, and for any of
/// them failing that, the description of its series.
fn title_of(obj: &InMemDicomObject, kind: &DocumentKind) -> String {
    let own = match kind {
        DocumentKind::Report => first_item(obj, tags::CONCEPT_NAME_CODE_SEQUENCE)
            .and_then(|code| text(code, tags::CODE_MEANING)),
        DocumentKind::Pdf => text(obj, tags::DOCUMENT_TITLE),
        DocumentKind::Other(_) => None,
    };
    own.or_else(|| text(obj, tags::SERIES_DESCRIPTION))
        .unwrap_or_default()
}

/// `documents` in the order of `studies`, then by entry name. Those of a study that holds no
/// images come after the others, study by study in the order of the studies' identifiers.
pub(crate) fn in_order(
    mut documents: Vec<IndexedDocument>,
    studies: &[StudyInfo],
) -> Vec<IndexedDocument> {
    documents.sort_by(|a, b| {
        (study_place(a, studies), &a.entry).cmp(&(study_place(b, studies), &b.entry))
    });
    documents
}

/// Where a document's study comes among `studies`; for a study that is not among them, after
/// them all, by its identifier.
fn study_place<'a>(document: &'a IndexedDocument, studies: &[StudyInfo]) -> (usize, &'a str) {
    let known = studies
        .iter()
        .position(|study| study.id.0 == document.study_uid);
    match known {
        Some(at) => (at, ""),
        None => (studies.len(), document.study_uid.as_str()),
    }
}

/// The PDF file held in a DICOM file of encapsulated PDF, as long as the file says it is: the
/// stored bytes are cut to the Encapsulated Document Length when the file gives one that is no
/// longer than what is stored, so a PDF whose last byte is really zero keeps it. When the file
/// gives none, or one that does not fit, one trailing zero byte is taken to be the pad the
/// standard adds to a document of odd length and is removed, which would also remove a last
/// byte of the PDF that is zero. Nothing else is changed. Voxa draws no PDF and starts no
/// program to show it; the bytes are for the person to save.
///
/// # Errors
///
/// [`VoxaError::NotDicom`] when the bytes are not a DICOM file, and [`VoxaError::Decode`] when
/// the file is damaged or cut short, or holds no PDF.
pub fn pdf_bytes(bytes: &[u8]) -> Result<Vec<u8>, VoxaError> {
    guarded(|| {
        let file = open(bytes)?;
        if sop_class_of(&file).as_deref() != Some(uids::ENCAPSULATED_PDF_STORAGE) {
            return Err(VoxaError::Decode(
                "this is not a PDF stored in DICOM".to_owned(),
            ));
        }
        let held = file
            .get(tags::ENCAPSULATED_DOCUMENT)
            .and_then(|element| element.to_bytes().ok())
            .ok_or_else(|| VoxaError::Decode("the file holds no document".to_owned()))?;
        let mut pdf = held.into_owned();
        match stated_length(&file).filter(|length| *length <= pdf.len()) {
            Some(length) => pdf.truncate(length),
            None if pdf.last() == Some(&0) => {
                pdf.pop();
            }
            None => {}
        }
        Ok(pdf)
    })
}

/// The length of the document that the file gives, without the pad.
fn stated_length(file: &DefaultDicomObject) -> Option<usize> {
    let length = file.get(tags::ENCAPSULATED_DOCUMENT_LENGTH)?;
    usize::try_from(length.to_int::<u64>().ok()?).ok()
}

#[cfg(test)]
mod report_tests;
#[cfg(test)]
mod tests;
