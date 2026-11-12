//! Opening label files from the window: one named when Volsa was started, one dropped on the
//! window, and remembering which is open so that another table of names can be chosen for it.

use std::path::PathBuf;
use std::sync::Arc;

use egui_wgpu::RenderState;
use volsa_labels::LabelSet;
use volsa_render::{LabelsOnDevice, PartLook};

use super::scene::export::said;
use super::{VolsaApp, parts, theme};
use crate::action::Action;
use crate::export::write;
use crate::labels::{LABELS_WAIT, Labels, NOTHING_TO_LABEL_KEY, is_label_file, on_this_series};
use crate::parts_table::{PARTS_NAME, parts_csv};
use crate::planes::grid;
use crate::scene::SceneStatus;
use crate::state::{AppState, Notice, NoticeKind};

/// The label file waiting to be opened, and the one last opened. Neither is kept anywhere
/// but here, for as long as the window is open.
#[derive(Debug, Default)]
pub struct Opening {
    /// A label file named when Volsa was started, until there is a volume to open it on.
    waiting: Option<PathBuf>,
    /// The label file last opened.
    opened: Option<PathBuf>,
    /// Whether it has been said that the file named at the start is waiting.
    said_waiting: bool,
}

impl Opening {
    /// With a label file to open as soon as it can be.
    pub fn waiting_for(labels: Option<PathBuf>) -> Self {
        Self {
            waiting: labels,
            opened: None,
            said_waiting: false,
        }
    }

    /// Whether a label file named when Volsa was started has still to be opened.
    pub fn is_waiting(&self) -> bool {
        self.waiting.is_some()
    }

    /// What to say, once, while a label file named when Volsa was started waits and the
    /// series shown has no 3D view to open it on. Nothing while a 3D view may yet come.
    pub fn waiting_notice(&mut self, state: &AppState) -> Option<Notice> {
        let no_view = matches!(state.scene.status, SceneStatus::Unavailable(_));
        if self.waiting.is_none() || !no_view || self.said_waiting {
            return None;
        }
        self.said_waiting = true;
        Some(Notice {
            kind: NoticeKind::Info,
            text: LABELS_WAIT.to_owned(),
            details: Vec::new(),
            key: Some(NOTHING_TO_LABEL_KEY),
        })
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
    /// Ask the person for the file the Labels and Tools tabs want, and give what comes of it:
    /// what opens a label file, or another table of names for the labels open, or what says
    /// how saving the list of `open` labels went. `None` when they chose nothing.
    pub fn chosen(&mut self, asked: parts::Asked, open: &Labels) -> Option<Action> {
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
            parts::Asked::SaveCsv => {
                let path = rfd::FileDialog::new()
                    .set_title("Save the parts as a table")
                    .set_file_name(format!("{PARTS_NAME}.csv"))
                    .add_filter("CSV table", &["csv"])
                    .save_file()?;
                let outcome = write(&path, parts_csv(open, theme::DISCLAIMER).as_bytes());
                Some(Action::Post(said(&path, outcome)))
            }
            parts::Asked::Labelling(wants) => parts::labelling::chosen(wants),
        }
    }
}

/// How every part looks, and which is selected: what the graphics device is told.
type Looks = (Vec<PartLook>, Option<usize>);

/// Whether these looks differ from the ones last sent to the device. They are kept as the
/// ones sent, so the same looks asked about twice are fresh once.
fn fresh(sent: &mut Option<Looks>, now: Looks) -> bool {
    if sent.as_ref() == Some(&now) {
        return false;
    }
    *sent = Some(now);
    true
}

/// What is said when the graphics device has no room for the labels.
pub fn no_room() -> Notice {
    let text = "The graphics device has no room for these labels: they are listed, and not drawn.";
    Notice {
        kind: NoticeKind::Warning,
        text: text.to_owned(),
        details: Vec::new(),
        key: Some("labels"),
    }
}

/// The labels on the graphics device as the window keeps them from one frame to the next:
/// one copy, which the plane views and the 3D scene both draw from.
#[derive(Default)]
pub struct Held {
    placed: Option<(Arc<LabelSet>, LabelsOnDevice)>,
    /// A set the device had no room for, so that it is not tried again every frame.
    refused: Option<Arc<LabelSet>>,
    /// The looks last sent to the device.
    sent: Option<Looks>,
    /// How many times what the device holds has changed: labels put there or taken off,
    /// or looks sent. A picture drawn from them is out of date when this has moved on.
    stamp: u64,
}

impl Held {
    /// Keep on the device the labels that are drawn on the series in the scene, and tell it
    /// how each part looks whenever that has changed. `set` is the label set a worker read;
    /// labels of another study than the one shown are taken off the device. True when the
    /// device had no room for the labels, said once for each set.
    pub fn keep(
        &mut self,
        render: Option<&RenderState>,
        state: &AppState,
        set: Option<&Arc<LabelSet>>,
    ) -> bool {
        let Some((set, render)) = set.filter(|_| on_this_series(state)).zip(render) else {
            self.stamp += u64::from(self.placed.is_some());
            (self.placed, self.sent) = (None, None);
            return false;
        };
        let same = |held: &Arc<LabelSet>| Arc::ptr_eq(held, set);
        if self.placed.as_ref().is_some_and(|(held, _)| !same(held)) {
            (self.placed, self.sent) = (None, None);
        }
        let mut no_room = false;
        if self.placed.is_none() && !self.refused.as_ref().is_some_and(same) {
            let placed = LabelsOnDevice::put(&render.device, &render.queue, set);
            no_room = placed.is_none();
            self.refused = no_room.then(|| Arc::clone(set));
            self.placed = placed.map(|placed| (Arc::clone(set), placed));
            self.sent = None;
        }
        let looks = (state.labels.looks(), state.labels.selected);
        if let Some((_, placed)) = &self.placed
            && fresh(&mut self.sent, looks)
            && let Some((looks, selected)) = &self.sent
        {
            placed.set_looks(&render.queue, looks, *selected);
            self.stamp += 1;
        }
        no_room
    }

    /// A number that moves on whenever what the device holds of the labels changes.
    pub fn stamp(&self) -> u64 {
        self.stamp
    }

    /// The labels on the device, if there are any to draw.
    pub fn on_device(&self) -> Option<&LabelsOnDevice> {
        self.placed.as_ref().map(|(_, placed)| placed)
    }
}

impl VolsaApp {
    /// Open the label file named when Volsa was started, once the scan it was named with has
    /// a volume ready to put labels on; and say so, once, while it waits on a series that
    /// has no 3D view.
    pub(super) fn open_waiting_labels(&mut self) {
        if let Some(waiting) = self.labels.waiting_notice(self.session.state()) {
            self.dispatch(Action::Post(waiting));
        }
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
    fn labels_named_at_the_start_say_once_that_they_wait_for_a_series_with_a_3d_view() {
        use crate::scene::SceneStatus;
        let no_view = || SceneStatus::Unavailable(vec!["why".to_owned()]);
        let mut state = AppState::default();
        let mut opening = Opening::waiting_for(Some(PathBuf::from("labels.nii.gz")));
        assert_eq!(opening.waiting_notice(&state), None, "nothing is shown yet");
        state.scene.status = SceneStatus::Building { done: 1, of: 9 };
        assert_eq!(
            opening.waiting_notice(&state),
            None,
            "a 3D view is on its way"
        );
        state.scene.status = no_view();
        let notice = opening.waiting_notice(&state).expect("said");
        assert_eq!(notice.text, LABELS_WAIT);
        assert_eq!(
            notice.key,
            Some(NOTHING_TO_LABEL_KEY),
            "it goes when labels are opened"
        );
        assert!(LABELS_WAIT.contains("waiting") && LABELS_WAIT.contains("3D view"));
        assert_eq!(opening.waiting_notice(&state), None, "said once");
        let mut none_named = Opening::default();
        assert_eq!(
            none_named.waiting_notice(&state),
            None,
            "no labels were named"
        );
    }

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

    #[test]
    fn the_looks_are_sent_to_the_device_once_and_again_only_when_they_change() {
        let look = |opacity| volsa_render::PartLook {
            colour: [1, 2, 3],
            opacity,
        };
        let mut sent = None;
        assert!(fresh(&mut sent, (vec![look(1.0)], None)), "the first time");
        assert!(
            !fresh(&mut sent, (vec![look(1.0)], None)),
            "nothing has changed"
        );
        assert!(
            fresh(&mut sent, (vec![look(0.0)], None)),
            "a part was hidden"
        );
        assert!(fresh(&mut sent, (vec![look(0.5)], None)), "or faded");
        assert!(fresh(&mut sent, (vec![look(0.5)], Some(0))), "or selected");
        assert!(!fresh(&mut sent, (vec![look(0.5)], Some(0))));
        assert_eq!(
            sent,
            Some((vec![look(0.5)], Some(0))),
            "what was sent last is kept"
        );
    }
}
