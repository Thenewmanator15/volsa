//! Documents of the export: opening one in the middle of the window, taking in what a worker
//! read of it, closing it, and the one piece of work that saving a PDF calls for.

use voxa_dicom::{DocumentKind, IndexedDocument};

use crate::action::{Action, DocumentAction, Effect};
use crate::state::{AppState, OpenDocument};

/// Whether an action is about the document open.
pub(super) fn concerns(action: &Action) -> bool {
    matches!(action, Action::Document(_))
}

pub(super) fn reduce(state: &mut AppState, action: Action) -> Vec<Effect> {
    match action {
        Action::Document(DocumentAction::Open(entry)) => return open(state, entry),
        Action::Document(DocumentAction::Close) => state.document = None,
        Action::Document(DocumentAction::Read { entry, report }) => {
            // What was read for a document that is no longer the one open is let go.
            let open = state.document.as_mut().filter(|open| open.entry == entry);
            if let Some(open) = open {
                open.report = Some(report);
            }
        }
        // Only a PDF has a copy to save.
        Action::Document(DocumentAction::SavePdf(entry, to))
            if listed(state, &entry).is_some_and(|one| one.kind == DocumentKind::Pdf) =>
        {
            return vec![Effect::SaveDocumentPdf { entry, to }];
        }
        _ => {}
    }
    Vec::new()
}

/// The document of the export open that is stored in this entry.
fn listed<'a>(state: &'a AppState, entry: &str) -> Option<&'a IndexedDocument> {
    let index = state.index.as_ref()?;
    index
        .documents
        .iter()
        .find(|document| document.entry == entry)
}

/// Open the document in `entry`, and ask for a report to be read as text. A document that is
/// not in the export is not opened. The one already open is left as it is, unless it is a
/// report that could not be read, which is read again.
fn open(state: &mut AppState, entry: String) -> Vec<Effect> {
    let Some(kind) = listed(state, &entry).map(|document| document.kind.clone()) else {
        return Vec::new();
    };
    let failed = |open: &OpenDocument| matches!(open.report, Some(Err(_)));
    let already = state.document.as_ref();
    if already.is_some_and(|open| open.entry == entry && !failed(open)) {
        return Vec::new();
    }
    state.document = Some(OpenDocument {
        entry: entry.clone(),
        report: None,
    });
    match kind {
        DocumentKind::Report => vec![Effect::ReadDocument { entry }],
        DocumentKind::Pdf | DocumentKind::Other(_) => Vec::new(),
    }
}
