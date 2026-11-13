//! Labelled parts in the state: the labels open on a study, which of their parts are shown
//! and how solid each is, and which one is selected.

use volsa_core::{Patient, Point, SeriesId, StudyId};
use volsa_labels::{Made, Part, Verdict};
use volsa_render::{Look, Overlay, PartLook};

use crate::state::AppState;

/// What is said when labels turn out to lie nowhere near the series they were opened on.
pub const NOWHERE_NEAR: &str =
    "These labels lie nowhere near this series. They were probably made from another scan.";
/// What is said when a label file is opened and there is no volume to put its labels on.
pub const NOTHING_TO_LABEL: &str =
    "Labels are put on the 3D view of a scan. Open the scan first, then its labels.";
/// What is said when the labels named as Volsa was started cannot be opened yet.
pub const LABELS_WAIT: &str = "The labels named when Volsa was started are waiting: this series \
                               has no 3D view to put them on. They open on the first series \
                               shown that has one.";
/// A notice about labels there is nothing to put on takes the place of an earlier one,
/// and goes when labels are opened.
pub const NOTHING_TO_LABEL_KEY: &str = "nothing to label";
/// The least solid a part can be faded to. Hiding a part is done by its switch.
pub const FAINTEST: f32 = 0.1;

/// Whether there are labels, and if not, why.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LabelsStatus {
    /// None are open.
    #[default]
    None,
    /// A worker is reading them.
    Reading,
    /// They are open.
    Ready,
    /// They could not be opened; the sentence says why.
    Failed(String),
}

/// How one part is shown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PartShown {
    /// Whether it is shown at all.
    pub shown: bool,
    /// How solid it is when shown, from a tenth to 1.
    pub opacity: f32,
}

/// Where the names of the parts came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NamesFrom {
    /// A table of names found beside the labels, or chosen.
    Table,
    /// The list of the model that made the labels, in a run of Volsa's own.
    Model,
    /// Nowhere: the parts are called after their numbers.
    #[default]
    Nothing,
}

/// The labels open, and what is shown of them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Labels {
    /// Whether there are any.
    pub status: LabelsStatus,
    /// What the labels are of, as their table says; empty when it does not.
    pub title: String,
    /// The parts, in the order of their table.
    pub parts: Vec<Part>,
    /// How each part is shown, in the same order.
    pub shown: Vec<PartShown>,
    /// The part selected, by its place in `parts`.
    pub selected: Option<usize>,
    /// How the labels are drawn over the plane views.
    pub overlay: Overlay,
    /// The labelling model's own check, when the table carries one.
    pub verdict: Option<Verdict>,
    /// The study the labels were opened on. They are shown on its series and on no other.
    pub study: Option<StudyId>,
    /// The corners of the label volume's box in the patient.
    pub corners: Option<[Point<Patient>; 8]>,
    /// Where the parts' names came from.
    pub names_from: NamesFrom,
    /// Who made the labels, when their table says; `None` for labels made elsewhere.
    pub made: Option<Made>,
}

impl Labels {
    /// How each part is to look on the graphics device: its colour, and how solid it is,
    /// which is not at all for a part that is hidden.
    pub fn looks(&self) -> Vec<PartLook> {
        let look = |(part, how): (&Part, &PartShown)| PartLook {
            colour: part.colour,
            opacity: if how.shown { how.opacity } else { 0.0 },
        };
        self.parts.iter().zip(&self.shown).map(look).collect()
    }

    /// Whether a part is there and shown.
    pub fn is_shown(&self, part: usize) -> bool {
        self.shown.get(part).is_some_and(|how| how.shown)
    }
}

/// Whether a file is opened as labels, by its name: a NIfTI file, plain or gzipped, whatever
/// the case of its letters. Anything else dropped on the window is a scan.
pub fn is_label_file(path: &std::path::Path) -> bool {
    let ending = |path: &std::path::Path, wanted: &str| {
        let ending = path.extension().map(|ending| ending.to_string_lossy());
        ending.is_some_and(|ending| ending.eq_ignore_ascii_case(wanted))
    };
    let unzipped = path.file_stem().map(std::path::Path::new);
    ending(path, "nii")
        || (ending(path, "gz") && unzipped.is_some_and(|inner| ending(inner, "nii")))
}

/// A part's name as it is said to the pointer: with its side, if it has one.
pub fn name_of(part: &Part) -> String {
    let side = match part.side {
        volsa_labels::Side::Left => ", left",
        volsa_labels::Side::Right => ", right",
        volsa_labels::Side::Midline => "",
    };
    format!("{}{side}", part.name)
}

/// The study a series belongs to.
pub fn study_of(state: &AppState, series: &SeriesId) -> Option<StudyId> {
    let studies = &state.index.as_ref()?.studies;
    let holds = |study: &&volsa_core::StudyInfo| study.series.iter().any(|one| one.id == *series);
    studies.iter().find(holds).map(|study| study.id.clone())
}

/// Whether there are labels to show on the series in the scene: they are open, and that
/// series is of the study they were opened on.
pub fn on_this_series(state: &AppState) -> bool {
    let study = state
        .scene
        .series
        .as_ref()
        .and_then(|series| study_of(state, series));
    state.labels.status == LabelsStatus::Ready && study.is_some() && study == state.labels.study
}

/// Whether the thin dark tubes of the scan in the scene can be highlighted now; if not, why.
/// They can when the scene shows a series whose volume is built, and no labels are being read
/// or made.
///
/// # Errors
///
/// The sentence that says why not.
pub fn can_find_veins(state: &AppState) -> Result<(), &'static str> {
    if crate::planes::grid(state).is_none() {
        return Err("Highlighting needs a scan with a 3D view. Open one, or wait for its 3D view.");
    }
    if state.labels.status == LabelsStatus::Reading || state.labelling.running.is_some() {
        return Err("Labels are being made or opened. Wait for them to finish.");
    }
    Ok(())
}

/// The place among the parts of the veins found, when the labels on this series hold them.
pub fn veins_part(state: &AppState) -> Option<usize> {
    let parts = on_this_series(state).then_some(&state.labels.parts)?;
    parts
        .iter()
        .position(|part| part.name == volsa_segment::VEINS_PART)
}

/// The look the scene is drawn in: the one chosen, except that Parts with no labels on this
/// series, which has nothing to draw, is drawn as Surface.
pub fn look_drawn(state: &AppState) -> Look {
    match state.scene.look {
        Look::Parts if !on_this_series(state) => Look::Surface,
        chosen => chosen,
    }
}
