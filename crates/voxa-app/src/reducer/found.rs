//! What is said of the files indexing found and did not open: those that are not DICOM
//! images, and those refused unread.

use super::{REFUSED, SKIPPED};
use crate::state::{Notice, NoticeKind};

/// The notice of the files the export holds that are neither scans nor documents Voxa lists, in
/// words that do not read as scans being lost; the names are in its details, each by its last
/// part alone: the folders a file lies in can be named for the patient (S-UI-02).
pub(super) fn skipped_notice(skipped: &[String]) -> Option<Notice> {
    let text = match skipped.len() {
        0 => return None,
        1 => "1 file is not a scan or a document Voxa lists".to_owned(),
        several => format!("{several} files are not scans or documents Voxa lists"),
    };
    Some(Notice {
        kind: NoticeKind::Info,
        text,
        details: skipped
            .iter()
            .map(|path| last_part(path).to_owned())
            .collect(),
        key: Some(SKIPPED),
    })
}

/// A warning that names each file refused unread, such as one nested too deeply to read
/// safely, by its own name as the skipped files are named, and says why.
pub(super) fn refused_notice(refused: &[voxa_dicom::Refused]) -> Option<Notice> {
    let text = match refused.len() {
        0 => return None,
        1 => "1 file was not read".to_owned(),
        several => format!("{several} files were not read"),
    };
    let details = refused
        .iter()
        .map(|file| format!("{}: {}", last_part(&file.entry), file.reason))
        .collect();
    Some(Notice {
        kind: NoticeKind::Warning,
        text,
        details,
        key: Some(REFUSED),
    })
}

/// The last part of a path inside an export, the file's own name.
fn last_part(path: &str) -> &str {
    let mut parts = path.rsplit(['/', '\\']);
    parts.find(|part| !part.is_empty()).unwrap_or(path)
}
