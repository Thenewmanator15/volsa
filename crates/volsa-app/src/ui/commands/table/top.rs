//! The window itself: opening a scan, the scans shown or hidden, the other files shown, full
//! screen, the size of the interface, the theme, the keys and About, where the log is, the
//! tags of a file, and opening labels.

use egui::Key;

use super::super::{Command, Does, Place, Request, TagsOf, Why};
use super::{PLAIN, alone, ctrl};
use crate::action::{Action, DocumentAction};
use crate::naming::{Group, group_of};
use crate::state::AppState;
use crate::ui::theme::ThemeChoice;
use crate::ui::top::Pick;

/// The words each size of the interface is found by.
const SIZE: &[&str] = &["interface size", "scale", "text size"];

/// Why the other files cannot be shown: the export open holds none, or none is open. Its
/// documents are other files too.
fn no_other_files(state: &AppState) -> Option<Why> {
    let none = Why::Borrowed("There are no other files to show.");
    let Some(index) = &state.index else {
        return Some(none);
    };
    let other_series = index
        .series
        .iter()
        .any(|listed| group_of(&listed.info) == Group::Other);
    (!other_series && index.documents.is_empty()).then_some(none)
}

/// Whether the document open is a PDF, which is the one kind of document a copy can be saved of.
fn document_is_a_pdf(state: &AppState) -> bool {
    open_document_is(state, &volsa_dicom::DocumentKind::Pdf)
}

/// Whether the document open is of this kind.
fn open_document_is(state: &AppState, kind: &volsa_dicom::DocumentKind) -> bool {
    let open = state.document.as_ref();
    let held = state.index.as_ref().map(|index| &index.documents);
    let found = open
        .zip(held)
        .and_then(|(open, all)| all.iter().find(|document| document.entry == open.entry));
    found.is_some_and(|document| document.kind == *kind)
}

/// A size the interface can be drawn at, as the menu offers it.
const fn size(id: &'static str, label: &'static str, name: &'static str, at: f32) -> Command {
    Command {
        id,
        name,
        label,
        words: SIZE,
        place: Place::Menu,
        does: Does::Ask(Request::Size(at)),
        ..PLAIN
    }
}

/// The action that draws the window in `choice`, as the top bar's theme button offers it.
const fn theme(
    id: &'static str,
    label: &'static str,
    name: &'static str,
    words: &'static [&'static str],
    choice: ThemeChoice,
) -> Command {
    Command {
        id,
        name,
        label,
        words,
        place: Place::TopBar,
        does: Does::Ask(Request::Theme(choice)),
        ..PLAIN
    }
}

/// The entries. egui itself answers the keys that size the interface: they are listed here
/// for the help and for search, and the list's lookup leaves them to egui.
pub(super) const COMMANDS: &[Command] = &[
    Command {
        id: "open.folder",
        name: "Open a folder of DICOM files",
        label: "Folder…",
        words: &["open", "folder", "directory", "load", "import"],
        place: Place::TopBar,
        does: Does::Ask(Request::Open(Pick::Folder)),
        ..PLAIN
    },
    Command {
        id: "open.file",
        name: "Open a zip file or a DICOMDIR",
        label: "Zip file or DICOMDIR…",
        words: &["open", "zip", "dicomdir", "file", "load"],
        place: Place::TopBar,
        does: Does::Ask(Request::Open(Pick::File)),
        ..PLAIN
    },
    Command {
        id: "rail.fold",
        name: "Show or hide the scans",
        label: "Scans",
        explains: "The column of scans at the left",
        words: &["scans", "series", "list", "sidebar", "hide", "fold"],
        key: Some(alone(Key::B, "B")),
        place: Place::Rail,
        does: Does::Ask(Request::FoldRail),
        ..PLAIN
    },
    Command {
        id: "rail.other",
        name: "Show the other files",
        label: "Other files",
        explains: "Locating images, scanned pages, screenshots and documents, under the scans",
        words: &[
            "other files",
            "pages",
            "documents",
            "screenshots",
            "locating",
            "scout",
        ],
        place: Place::Rail,
        why_not: no_other_files,
        does: Does::Ask(Request::ShowOtherFiles),
        ..PLAIN
    },
    Command {
        id: "window.full_screen",
        name: "Full screen, or back to a window",
        label: "Full screen",
        words: &["fullscreen", "maximise", "whole screen"],
        key: Some(alone(Key::F11, "F11")),
        place: Place::Menu,
        does: Does::Ask(Request::FullScreen),
        ..PLAIN
    },
    Command {
        id: "size.larger",
        name: "Draw the interface larger",
        label: "Larger",
        words: &["zoom in", "bigger", "enlarge"],
        key: Some(ctrl(Key::Equals, "Ctrl+=")),
        does: Does::Ask(Request::Larger),
        ..PLAIN
    },
    Command {
        id: "size.smaller",
        name: "Draw the interface smaller",
        label: "Smaller",
        words: &["zoom out", "smaller", "shrink"],
        key: Some(ctrl(Key::Minus, "Ctrl+-")),
        does: Does::Ask(Request::Smaller),
        ..PLAIN
    },
    size("size.75", "75%", "Draw the interface at 75%", 0.75),
    size("size.90", "90%", "Draw the interface at 90%", 0.9),
    // Written out, not made by `size`: what a struct update leaves of a call's result would
    // be dropped at compile time, which a request that can hold a series cannot be.
    Command {
        id: "size.100",
        name: "Draw the interface at its usual size",
        label: "100%",
        words: SIZE,
        key: Some(ctrl(Key::Num0, "Ctrl+0")),
        place: Place::Menu,
        does: Does::Ask(Request::Size(1.0)),
        ..PLAIN
    },
    size("size.110", "110%", "Draw the interface at 110%", 1.1),
    size("size.125", "125%", "Draw the interface at 125%", 1.25),
    size("size.150", "150%", "Draw the interface at 150%", 1.5),
    size("size.175", "175%", "Draw the interface at 175%", 1.75),
    size("size.200", "200%", "Draw the interface at 200%", 2.0),
    Command {
        id: "search.open",
        name: "Search actions, parts, scans and help",
        label: "Search",
        explains: "Type a few letters of what you want to do or find",
        words: &["find", "search", "look up", "go to", "command"],
        // A slash does it too: see the other keys in the list's own module.
        key: Some(ctrl(Key::K, "Ctrl+K or /")),
        place: Place::TopBar,
        does: Does::Ask(Request::Search),
        ..PLAIN
    },
    Command {
        id: "help.keys",
        name: "Show help, the keys and what the mouse does",
        label: "?",
        words: &["help", "topics", "shortcuts", "keyboard", "mouse", "keys"],
        // A question mark is its own key for egui, whichever keyboard types it.
        key: Some(alone(Key::Questionmark, "?")),
        place: Place::TopBar,
        does: Does::Ask(Request::Help(None)),
        ..PLAIN
    },
    theme(
        "theme.system",
        "System",
        "Follow the system's light or dark setting",
        &["theme", "appearance", "automatic", "system"],
        ThemeChoice::System,
    ),
    theme(
        "theme.light",
        "Light",
        "Use the light theme",
        &["theme", "appearance", "light mode", "bright"],
        ThemeChoice::Light,
    ),
    theme(
        "theme.dark",
        "Dark",
        "Use the dark theme",
        &["theme", "appearance", "dark mode", "night"],
        ThemeChoice::Dark,
    ),
    Command {
        id: "about",
        name: "About Volsa",
        label: "About",
        words: &["version", "licence", "credits", "disclaimer"],
        place: Place::Menu,
        does: Does::Ask(Request::About),
        ..PLAIN
    },
    Command {
        id: "log.where",
        name: "Show where the log is",
        label: "Where the log is",
        explains: "The log holds nothing about a patient and none of your files' names",
        words: &["log", "diagnostics", "support", "crash"],
        place: Place::Menu,
        does: Does::Ask(Request::WhereTheLogIs),
        ..PLAIN
    },
    Command {
        id: "notices.show",
        name: "Show the notices",
        label: "Notices",
        explains: "What Volsa has to tell you, such as how saving went",
        words: &["notices", "messages", "warnings", "skipped", "news"],
        place: Place::Line,
        why_not: |state| {
            state
                .notices
                .is_empty()
                .then_some(Why::Borrowed("There are no notices."))
        },
        does: Does::Ask(Request::ShowNotices),
        ..PLAIN
    },
    Command {
        id: "about.notices",
        name: "Show the licences and notices",
        label: "Licences and notices…",
        explains: "The licence and notice texts of everything Volsa is built from",
        words: &["licence", "license", "credits", "third party", "notices"],
        place: Place::About,
        does: Does::Ask(Request::Licences),
        ..PLAIN
    },
    Command {
        id: "log.copy",
        name: "Copy where the log is",
        label: "Copy",
        words: &["log", "path", "clipboard", "copy", "support"],
        place: Place::About,
        why_not: |_| {
            crate::log::path()
                .is_none()
                .then_some(Why::Borrowed("Volsa is not keeping a log in this run."))
        },
        does: Does::Ask(Request::CopyTheLog),
        ..PLAIN
    },
    Command {
        id: "document.close",
        name: "Close the document",
        label: "Close",
        explains: "Go back to the pictures",
        words: &["document", "report", "page", "close", "pictures"],
        place: Place::Document,
        why_not: |state| {
            state
                .document
                .is_none()
                .then_some(Why::Borrowed("No document is open."))
        },
        does: Does::Act(|_| Some(Action::Document(DocumentAction::Close))),
        ..PLAIN
    },
    Command {
        id: "document.save_pdf",
        name: "Save a copy as PDF…",
        label: "Save a copy as PDF…",
        explains: "Volsa does not show PDFs; this writes a copy of it where you choose",
        words: &["pdf", "save", "copy", "export", "document", "report"],
        place: Place::Document,
        why_not: |state| {
            if state.document.is_none() {
                return Some(Why::Borrowed("No document is open."));
            }
            (!document_is_a_pdf(state))
                .then_some(Why::Borrowed("Only a PDF can be saved as a copy."))
        },
        // The document it is the copy of is the one open: see [`Does::run`].
        does: Does::Ask(Request::SaveDocumentPdf(String::new())),
        ..PLAIN
    },
    Command {
        id: "document.open_labels",
        name: "Open the segmentation as labels",
        label: "Open as labels",
        explains: "Show its segments as labels over the 3D view of the scan shown",
        words: &[
            "segmentation",
            "seg",
            "labels",
            "segments",
            "open",
            "document",
        ],
        place: Place::Document,
        why_not: |state| {
            if state.document.is_none() {
                return Some(Why::Borrowed("No document is open."));
            }
            let segmentation = &volsa_dicom::DocumentKind::Segmentation;
            (!open_document_is(state, segmentation))
                .then_some(Why::Borrowed("Only a segmentation opens as labels."))
        },
        does: Does::Act(|state| {
            let open = state.document.as_ref()?;
            Some(Action::Document(DocumentAction::OpenAsLabels(
                open.entry.clone(),
            )))
        }),
        ..PLAIN
    },
    Command {
        id: "tags.show",
        name: "Show the file's tags",
        label: "Tags",
        explains: "Every element of the file behind the slice shown",
        words: &["tags", "dicom", "header", "elements", "metadata"],
        // On a card, the tags of the first file of that card's series.
        place: Place::Card,
        does: Does::Ask(Request::Tags(TagsOf::Shown)),
        ..PLAIN
    },
];
