//! What the person chose for labelling a scan, kept between runs of Voxa: which model, the
//! cortex areas, where a run goes, whether results are kept, and the two folders the models
//! and the runtime are in. One small file in the settings folder. The two folders are the
//! only paths in it, and it says nothing of any scan.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use voxa_models::{Card, CardKind, RunsOn};
use voxa_segment::ModelChoice;

use crate::kept_labels::write_whole;
use crate::labelling::Labelling;

/// The version of the file. A file of another version is not read.
pub const CHOICES_VERSION: u32 = 1;
/// The name of the file within the settings folder.
const FILE: &str = "choices.json";

/// Where a run was chosen to go, as it is kept. A card is kept by its name: the system's
/// identity for a card does not last until the next run of the program, and the card's place
/// in the system's list can change from one moment to the next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunsOnKept {
    /// Wherever it fits best.
    Automatic,
    /// On the processor.
    Processor,
    /// On the graphics card with this name.
    Card(String),
}

impl RunsOnKept {
    /// A place as it is kept, by the name `cards` (the system's list now) gives the card. A
    /// card that is not in the list has no name to be kept by, and is kept as automatic.
    pub fn of(runs_on: RunsOn, cards: &[Card]) -> Self {
        match runs_on {
            RunsOn::Automatic => Self::Automatic,
            RunsOn::Processor => Self::Processor,
            RunsOn::Card(id) => cards
                .iter()
                .find(|card| card.id == id)
                .map_or(Self::Automatic, |card| Self::Card(card.name.clone())),
        }
    }

    /// The place kept, as the system's list `cards` has it now: a card by the first real card
    /// of its name, which is any but the system drawing with the processor. With no card of
    /// that name the place is automatic.
    pub fn find(&self, cards: &[Card]) -> RunsOn {
        match self {
            Self::Automatic => RunsOn::Automatic,
            Self::Processor => RunsOn::Processor,
            Self::Card(name) => cards
                .iter()
                .find(|card| card.kind != CardKind::Software && card.name == *name)
                .map_or(RunsOn::Automatic, |card| RunsOn::Card(card.id)),
        }
    }
}

/// What is chosen for labelling, as it is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choices {
    /// Which model has been asked for.
    pub choice: ModelChoice,
    /// Whether the cortex is divided into its areas.
    pub cortex: bool,
    /// Where a run goes.
    pub runs_on: RunsOnKept,
    /// Whether results are kept on this computer.
    pub keep: bool,
    /// The folder of the models, when one was chosen.
    pub models: Option<PathBuf>,
    /// The folder of the runtime, when one was chosen.
    pub runtime: Option<PathBuf>,
}

impl Default for Choices {
    /// What Voxa starts with when nothing is kept.
    fn default() -> Self {
        Self {
            choice: ModelChoice::Automatic,
            cortex: true,
            runs_on: RunsOnKept::Automatic,
            keep: true,
            models: None,
            runtime: None,
        }
    }
}

impl Choices {
    /// What the state has chosen. A card chosen is kept by the name the state's list of
    /// cards gives it; one kept from the run before and not yet looked for in the list is
    /// kept as it was.
    pub fn of(labelling: &Labelling) -> Self {
        let runs_on = match (labelling.runs_on, &labelling.card_kept) {
            (RunsOn::Automatic, Some(name)) => RunsOnKept::Card(name.clone()),
            (runs_on, _) => RunsOnKept::of(runs_on, &labelling.cards),
        };
        Self {
            choice: labelling.choice,
            cortex: labelling.cortex,
            runs_on,
            keep: labelling.keep,
            models: labelling.models.clone(),
            runtime: labelling.runtime.clone(),
        }
    }

    /// Put these choices in the state as Voxa starts. No card has been listed yet, so a card
    /// kept by its name waits in the state, with the place left automatic, until the list
    /// comes and the card can be found in it.
    pub fn put_in(self, labelling: &mut Labelling) {
        labelling.choice = self.choice;
        labelling.cortex = self.cortex;
        labelling.keep = self.keep;
        labelling.models = self.models;
        labelling.runtime = self.runtime;
        (labelling.runs_on, labelling.card_kept) = match self.runs_on {
            RunsOnKept::Automatic => (RunsOn::Automatic, None),
            RunsOnKept::Processor => (RunsOn::Processor, None),
            RunsOnKept::Card(name) => (RunsOn::Automatic, Some(name)),
        };
    }
}

/// The file, with each choice as a word or a value of its own. A part the file lacks, or says
/// in words Voxa does not know, is the usual choice for it.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Written {
    version: u32,
    choice: String,
    cortex: bool,
    runs_on: String,
    card: Option<String>,
    keep: bool,
    models: Option<String>,
    runtime: Option<String>,
}

impl Default for Written {
    /// Version 0, which no file Voxa wrote has, with the usual choices.
    fn default() -> Self {
        Self {
            version: 0,
            ..Self::of(&Choices::default())
        }
    }
}

impl Written {
    fn of(choices: &Choices) -> Self {
        let choice = match choices.choice {
            ModelChoice::Automatic => "automatic",
            ModelChoice::Standard => "standard",
            ModelChoice::Robust => "robust",
        };
        let (runs_on, card) = match &choices.runs_on {
            RunsOnKept::Automatic => ("automatic", None),
            RunsOnKept::Processor => ("processor", None),
            RunsOnKept::Card(name) => ("card", Some(name.clone())),
        };
        // A folder whose name is not text cannot be written, and is not kept.
        let folder = |folder: &Option<PathBuf>| {
            let text = folder.as_deref().and_then(Path::to_str);
            text.map(str::to_owned)
        };
        Self {
            version: CHOICES_VERSION,
            choice: choice.to_owned(),
            cortex: choices.cortex,
            runs_on: runs_on.to_owned(),
            card,
            keep: choices.keep,
            models: folder(&choices.models),
            runtime: folder(&choices.runtime),
        }
    }

    fn into_choices(self) -> Choices {
        let choice = match self.choice.as_str() {
            "standard" => ModelChoice::Standard,
            "robust" => ModelChoice::Robust,
            _ => ModelChoice::Automatic,
        };
        let card = self.card.filter(|name| !name.is_empty());
        let runs_on = match (self.runs_on.as_str(), card) {
            ("processor", _) => RunsOnKept::Processor,
            ("card", Some(name)) => RunsOnKept::Card(name),
            _ => RunsOnKept::Automatic,
        };
        let folder = |text: Option<String>| text.filter(|text| !text.is_empty()).map(PathBuf::from);
        Choices {
            choice,
            cortex: self.cortex,
            runs_on,
            keep: self.keep,
            models: folder(self.models),
            runtime: folder(self.runtime),
        }
    }
}

/// The choices kept in `folder`. A file that is missing, cannot be read, is not what Voxa
/// wrote or is of another version gives the choices Voxa starts with.
pub fn load_choices(folder: &Path) -> Choices {
    let text = std::fs::read_to_string(folder.join(FILE)).unwrap_or_default();
    match serde_json::from_str::<Written>(&text) {
        Ok(written) if written.version == CHOICES_VERSION => written.into_choices(),
        _ => Choices::default(),
    }
}

/// Keep these choices in `folder`, in place of any kept before. The folder is made if it is
/// not there.
///
/// # Errors
///
/// When the folder cannot be made or the file cannot be written.
pub fn save_choices(folder: &Path, choices: &Choices) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(&Written::of(choices))?;
    std::fs::create_dir_all(folder)?;
    write_whole(&folder.join(FILE), text.as_bytes())
}

#[cfg(test)]
mod tests;
