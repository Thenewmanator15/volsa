//! The documents of an export: which of them the rail lists, in what order, and how they are
//! called. A document is a DICOM file that holds no image, such as a structured report or a
//! PDF; the index lists each with the study it says it belongs to, and the rail lists it with
//! the other files of that study.

use volsa_dicom::{DocumentKind, Index, IndexedDocument};

use crate::naming::printable;
use crate::people::people_of;

pub mod mask;

pub use mask::Mask;

/// The places in [`Index::documents`] of the documents of the study at `study` of
/// [`Index::studies`], in the order the index lists them.
pub fn of_study(index: &Index, study: usize) -> Vec<usize> {
    let Some(study) = index.studies.get(study) else {
        return Vec::new();
    };
    let documents = index.documents.iter().enumerate();
    let theirs = documents.filter(|(_, document)| document.study_uid == study.id.0);
    theirs.map(|(place, _)| place).collect()
}

/// The places in [`Index::documents`] of the documents of studies that hold no images, in the
/// order the index lists them.
pub fn unplaced(index: &Index) -> Vec<usize> {
    let documents = index.documents.iter().enumerate();
    let known = |document: &IndexedDocument| {
        let mut studies = index.studies.iter();
        studies.any(|study| study.id.0 == document.study_uid)
    };
    let alone = documents.filter(|(_, document)| !known(document));
    alone.map(|(place, _)| place).collect()
}

/// The first document as the rail lists them: person by person, study by study as the rail
/// lists the studies, then those of studies that hold no images.
pub fn first(index: &Index) -> Option<&IndexedDocument> {
    let people = people_of(index, false);
    let mut studies = people.iter().flat_map(|person| person.studies.iter());
    let in_a_study = studies.find_map(|study| of_study(index, *study).first().copied());
    let place = in_a_study.or_else(|| unplaced(index).first().copied());
    index.documents.get(place?)
}

/// What a document is, in words: "Report", "Report (PDF)", "Segmentation", or for any other
/// file what the standard calls it, such as "Key-image note". Without a control or format
/// character.
pub fn kind_words(kind: &DocumentKind) -> String {
    match kind {
        DocumentKind::Report => "Report".to_owned(),
        DocumentKind::Pdf => "Report (PDF)".to_owned(),
        DocumentKind::Segmentation => "Segmentation".to_owned(),
        DocumentKind::Other(words) => printable(words),
    }
}

#[cfg(test)]
mod tests;
