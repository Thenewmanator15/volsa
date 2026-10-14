//! Labelling a scan with Voxa's own networks, as the state keeps it: whether this computer can
//! do it, what has been chosen for it, and what is running.

use std::path::PathBuf;

use voxa_core::SeriesId;
use voxa_models::{Card, CardKind, RunsOn, Wanted};
use voxa_segment::{ModelChoice, Stage, THICK_MM, choose};

use crate::labels::LabelsStatus;
use crate::planes::grid;
use crate::scene::SceneStatus;
use crate::state::AppState;

/// Whether this computer can label a scan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Ready {
    /// Not looked at yet.
    #[default]
    Unknown,
    /// The models and the runtime are there and fit one another.
    Yes,
    /// Labelling cannot be done; the sentence says why, as the worker found it.
    No(String),
}

/// Everything that can happen to labelling a scan with Voxa's own networks: what the person
/// chooses, what a worker reports, and the start and the stop of a run. It is carried by
/// [`Action::Labelling`](crate::Action::Labelling).
#[derive(Debug, Clone, PartialEq)]
pub enum LabellingAction {
    /// Label the scan in the scene, if that can be done now.
    LabelScan {
        /// How many bytes of the graphics card Voxa draws on it holds itself.
        held: u64,
    },
    /// A labelling run has reached a stage.
    LabellingStage {
        /// Which run, by the number it was started with.
        run: u64,
        /// The stage it has reached.
        stage: Stage,
    },
    /// A labelling run failed.
    LabellingFailed {
        /// Which run, by the number it was started with.
        run: u64,
        /// Why, in a sentence.
        why: String,
    },
    /// Stop the labelling run.
    StopLabelling,
    /// Look again at whether this computer can label a scan, in the folders as they are.
    Look,
    /// A worker has looked at whether this computer can label a scan, and at its cards.
    LabellingLooked {
        /// Which look, by the number it was asked for with.
        look: u64,
        /// Whether it can.
        ready: Ready,
        /// Its graphics cards.
        cards: Vec<Card>,
    },
    /// The name of the graphics card Voxa draws on.
    DrawingOn(String),
    /// Choose which model labels the scan.
    SetModelChoice(ModelChoice),
    /// Choose whether the cortex is divided into its areas.
    SetCortex(bool),
    /// Choose where the run goes.
    SetRunsOn(RunsOn),
    /// Choose whether the result is kept.
    SetKeep(bool),
    /// Choose the folder the models are in.
    SetModelsFolder(PathBuf),
    /// Choose the folder the runtime is in.
    SetRuntimeFolder(PathBuf),
}

/// Labelling in the state.
#[derive(Debug, Clone, PartialEq)]
pub struct Labelling {
    /// Whether this computer can label a scan.
    pub ready: Ready,
    /// The graphics cards of this computer, as the worker listed them.
    pub cards: Vec<Card>,
    /// The name of the card Voxa draws on, if it draws on one.
    pub drawing_on: Option<String>,
    /// Which model has been asked for.
    pub choice: ModelChoice,
    /// Whether the cortex is divided into its areas.
    pub cortex: bool,
    /// Where the person wants the run to go.
    pub runs_on: RunsOn,
    /// Whether the result is kept, to open again later.
    pub keep: bool,
    /// The folder of the models, when the person has chosen one.
    pub models: Option<PathBuf>,
    /// The folder of the runtime, when the person has chosen one.
    pub runtime: Option<PathBuf>,
    /// How far a run has got; `None` when none is on.
    pub running: Option<Stage>,
    /// The series the run is labelling.
    pub on: Option<SeriesId>,
    /// The date of a kept result, as it is written for the person.
    pub kept: Option<String>,
    /// How many runs have been started: the number of the newest. What a worker says of any
    /// other run is old news.
    pub runs: u64,
    /// How many looks at this computer have been asked for: the number of the newest. What a
    /// worker found on an older look is old news.
    pub looks: u64,
}

impl Default for Labelling {
    fn default() -> Self {
        Self {
            ready: Ready::Unknown,
            cards: Vec::new(),
            drawing_on: None,
            choice: ModelChoice::Automatic,
            cortex: true,
            runs_on: RunsOn::Automatic,
            keep: true,
            models: None,
            runtime: None,
            running: None,
            on: None,
            kept: None,
            runs: 0,
            looks: 0,
        }
    }
}

/// What is said when this computer has not been looked at yet.
const NOT_CHECKED: &str = "Voxa has not yet checked whether this computer can label a scan.";
/// What is said when a run is on and another is asked for.
const ALREADY_RUNNING: &str =
    "A scan is already being labelled. Wait for it to finish, or stop it.";
/// What is said while a file of labels is being read: its labels would be taken for the run's.
const READING_LABELS: &str = "A label file is still being opened. Wait for it to finish.";
/// What is said while the volume of the scan is still being built.
const STILL_BUILDING: &str =
    "The 3D view of this scan is still being built. Labelling can start when it is ready.";
/// What is said when there is no volume to label and none is being built.
const NO_VOLUME: &str = "Labelling needs a scan with a 3D view. Open a scan that has one first.";
/// What is said of a card that was chosen and is not in the computer's list now.
const CARD_GONE: &str = "the graphics card you chose (it is not listed now)";
/// What is said when the model cannot be named, since no scan is there to pick it from.
const MODEL_UNPICKED: &str = "Which one runs is picked from how far apart the slices are.";

/// Whether a run can start now; if not, why, in a sentence a person can be shown. A run can
/// start when this computer can label a scan, none is running, no file of labels is being
/// read (what a worker brings back then would be taken for the run's), and the scene shows a
/// series whose volume is built. Whether the scan fits in memory is not asked here: it is found
/// when the run starts, and said as its failure.
///
/// # Errors
///
/// The sentence that says why not.
pub fn can_label(state: &AppState) -> Result<(), String> {
    let labelling = &state.labelling;
    match &labelling.ready {
        Ready::Yes => {}
        Ready::Unknown => return Err(NOT_CHECKED.to_owned()),
        Ready::No(why) => return Err(why.clone()),
    }
    if labelling.running.is_some() {
        return Err(ALREADY_RUNNING.to_owned());
    }
    if state.labels.status == LabelsStatus::Reading {
        return Err(READING_LABELS.to_owned());
    }
    if grid(state).is_some() {
        return Ok(());
    }
    let building = matches!(state.scene.status, SceneStatus::Building { .. });
    Err((if building { STILL_BUILDING } else { NO_VOLUME }).to_owned())
}

/// The line under the button: which model will run, with the cortex or without, and where.
/// With the model left to Voxa it also says which was picked and why. The place is only said
/// to be where Voxa will try first: whether the scan fits there is found when the run starts.
pub fn what_will_run(state: &AppState) -> String {
    let labelling = &state.labelling;
    let model = match robust(state) {
        Some(true) => "Robust",
        Some(false) => "Standard",
        None => "Standard or robust",
    };
    let cortex = if labelling.cortex {
        " with cortex areas"
    } else {
        ""
    };
    let place = place(labelling);
    let line = format!("{model} model{cortex}, on {place}.");
    if labelling.choice != ModelChoice::Automatic {
        return line;
    }
    let reason = match robust(state) {
        Some(true) => {
            format!("Robust was picked because the slices are more than {THICK_MM} mm apart.")
        }
        Some(false) => {
            format!("Standard was picked because the slices are no more than {THICK_MM} mm apart.")
        }
        None => MODEL_UNPICKED.to_owned(),
    };
    format!("{line} {reason}")
}

/// What the run is to do, as the choices and the scan in the scene give it. `None` when the
/// model is left to Voxa and there is no scan to pick it from.
pub(crate) fn wanted(state: &AppState) -> Option<Wanted> {
    Some(Wanted {
        robust: robust(state)?,
        cortex: state.labelling.cortex,
    })
}

/// Whether the robust model is the one to run. `None` when it is left to Voxa and the scene
/// shows no scan to pick it from.
fn robust(state: &AppState) -> Option<bool> {
    let choice = state.labelling.choice;
    let spacing = state
        .scene
        .series
        .as_ref()
        .and_then(|series| state.series(series))
        .map(|listed| listed.info.spacing_mm);
    match (choice, spacing) {
        (ModelChoice::Automatic, None) => None,
        (_, spacing) => Some(choose(spacing.unwrap_or_default(), choice)),
    }
}

/// Where the run will be tried first, as it is said: a card by its name, or the processor.
/// With the place left to Voxa that is the first discrete card, if there is one.
fn place(labelling: &Labelling) -> String {
    let named = |card: &Card| format!("the graphics card ({})", card.name);
    match labelling.runs_on {
        RunsOn::Processor => "the processor".to_owned(),
        RunsOn::Card(id) => labelling
            .cards
            .iter()
            .find(|card| card.id == id)
            .map_or_else(|| CARD_GONE.to_owned(), named),
        RunsOn::Automatic => labelling
            .cards
            .iter()
            .find(|card| card.kind == CardKind::Discrete)
            .map_or_else(|| "the processor".to_owned(), named),
    }
}
