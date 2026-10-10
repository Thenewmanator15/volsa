//! Opening label files from the window: one named when Voxa was started, one dropped on the
//! window, and remembering which is open so that another table of names can be chosen for it.

use std::path::PathBuf;

use super::{VoxaApp, parts};
use crate::action::Action;
use crate::labels::is_label_file;
use crate::planes::grid;

/// The label file waiting to be opened, and the one last opened. Neither is kept anywhere
/// but here, for as long as the window is open.
#[derive(Debug, Default)]
pub struct Opening {
    /// A label file named when Voxa was started, until there is a volume to open it on.
    waiting: Option<PathBuf>,
    /// The label file last opened.
    opened: Option<PathBuf>,
}

impl Opening {
    /// With a label file to open as soon as it can be.
    pub fn waiting_for(labels: Option<PathBuf>) -> Self {
        Self {
            waiting: labels,
            opened: None,
        }
    }

    /// Whether a label file named when Voxa was started has still to be opened.
    pub fn is_waiting(&self) -> bool {
        self.waiting.is_some()
    }

    /// What opens a file the person chose or dropped on the window: labels if its name says
    /// it is a label file, otherwise a scan.
    pub fn opening(&mut self, path: PathBuf) -> Action {
        if is_label_file(&path) {
            self.labels(path, None)
        } else {
            Action::OpenPath(path)
        }
    }

    /// What opens a label file, with a table of names if one was chosen for it.
    pub fn labels(&mut self, labels: PathBuf, table: Option<PathBuf>) -> Action {
        self.opened = Some(labels.clone());
        Action::OpenLabels { labels, table }
    }

    /// What opens the label file last opened again with another table of names. `None` when
    /// no label file has been opened.
    pub fn renamed(&mut self, table: PathBuf) -> Option<Action> {
        let labels = self.opened.clone()?;
        Some(self.labels(labels, Some(table)))
    }
}

impl Opening {
    /// Ask the person for the file the Parts list wants, and give what opens it: a label
    /// file, or another table of names for the labels open. `None` when they chose nothing.
    pub fn chosen(&mut self, asked: parts::Asked) -> Option<Action> {
        match asked {
            parts::Asked::OpenLabels => {
                let labels = rfd::FileDialog::new()
                    .set_title("Open a file of labels")
                    .add_filter("NIfTI label volume", &["nii", "gz"])
                    .pick_file()?;
                Some(self.labels(labels, None))
            }
            parts::Asked::OpenNames => {
                let table = rfd::FileDialog::new()
                    .set_title("Choose a table of names for these labels")
                    .add_filter("Table of names", &["json", "ctbl"])
                    .pick_file()?;
                self.renamed(table)
            }
        }
    }
}

impl VoxaApp {
    /// Open the label file named when Voxa was started, once the scan it was named with has
    /// a volume ready to put labels on.
    pub(super) fn open_waiting_labels(&mut self) {
        if self.labels.waiting.is_none() || grid(self.session.state()).is_none() {
            return;
        }
        if let Some(labels) = self.labels.waiting.take() {
            let open = self.labels.labels(labels, None);
            self.dispatch(open);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dropped_file_is_opened_as_labels_or_as_a_scan_by_its_name() {
        let mut opening = Opening::default();
        let scan = PathBuf::from("some/export.zip");
        assert_eq!(opening.opening(scan.clone()), Action::OpenPath(scan));
        assert_eq!(
            opening.renamed(PathBuf::from("names.json")),
            None,
            "no labels yet"
        );
        for name in ["head/labels.nii.gz", "head/LABELS.NII"] {
            let labels = PathBuf::from(name);
            let open = Action::OpenLabels {
                labels: labels.clone(),
                table: None,
            };
            assert_eq!(opening.opening(labels), open);
        }
        let renamed = Action::OpenLabels {
            labels: PathBuf::from("head/LABELS.NII"),
            table: Some(PathBuf::from("names.ctbl")),
        };
        assert_eq!(opening.renamed(PathBuf::from("names.ctbl")), Some(renamed));
        assert!(
            !is_label_file(&PathBuf::from("nii")) && !is_label_file(&PathBuf::from("a.nii.zip"))
        );
    }
}
