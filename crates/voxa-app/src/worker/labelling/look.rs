//! Looking at whether this computer can label a scan: the model found and checked, the
//! runtime loaded, and the cards listed. What the newest look found is kept and told; what an
//! older look found is neither, so that it cannot take the place of the newer.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use voxa_models::{Card, Model, ModelError, RuntimeError, read_model};

use super::Kit;
use super::turn::on_its_own;
use crate::action::Action;
use crate::labelling::{LabellingAction, Ready};
use crate::worker::{Shared, Workers, lock, shielded};

/// Said after anything that keeps this computer from labelling a scan: where to read what
/// goes where. A download of Voxa carries `docs/models.md` as `MODELS.md`, beside the program.
const WHERE: &str =
    "MODELS.md beside the program (docs/models.md in the source) says what goes where.";
/// The folders looked in beside the program.
const MODELS: &str = "models";
const RUNTIME: &str = "runtime";
/// What is said when a look stops part-way.
const PART_WAY: &str = "Voxa could not finish checking the model and the runtime.";
/// What is said when there is no model beside the program and no folder was chosen for it.
const NO_MODEL: &str = "Voxa found no model: there is no folder called models beside it, and \
                        none has been chosen. Choose the folder that holds the model, under \
                        Options.";
/// The same for the runtime.
const NO_RUNTIME: &str = "Voxa found no runtime: there is no folder called runtime beside it, \
                          and none has been chosen. Choose the folder that holds the runtime, \
                          under Options.";

impl Kit {
    /// Note that look `look` has been asked for: it is the newest.
    pub(super) fn asked_to_look(&self, look: u64) {
        self.looks.fetch_max(look, Ordering::SeqCst);
    }

    /// Keep what look `look` found, if it is the newest asked for: the model, while this
    /// computer can label a scan, and nothing otherwise. Whether it was the newest, and so is
    /// news to be told.
    pub(super) fn found(&self, look: u64, model: Option<Arc<Model>>) -> bool {
        let mut kept = lock(&self.model);
        let newest = self.looks.load(Ordering::SeqCst) == look;
        if newest {
            *kept = model;
        }
        newest
    }
}

impl Workers {
    /// Look, on a thread of its own, for the model and the runtime, the model first, and list
    /// the cards. Reading a model folder and loading the runtime each take seconds.
    pub(super) fn look(&self, look: u64, models: Option<PathBuf>, runtime: Option<PathBuf>) {
        self.shared.labelling.asked_to_look(look);
        let shared = Arc::clone(&self.shared);
        let started = on_its_own(move || {
            let mut found = (Ready::No(format!("{PART_WAY} {WHERE}")), None, Vec::new());
            shielded(|| found = shared.looked(models.as_deref(), runtime.as_deref()));
            shared.tell(look, found);
        });
        if !started {
            let not_ready = (Ready::No(format!("{PART_WAY} {WHERE}")), None, Vec::new());
            self.shared.tell(look, not_ready);
        }
    }
}

impl Shared {
    /// Keep and tell what look `look` found, if it is the newest asked for.
    fn tell(&self, look: u64, (ready, model, cards): (Ready, Option<Arc<Model>>, Vec<Card>)) {
        if self.labelling.found(look, model) {
            let looked = LabellingAction::LabellingLooked { look, ready, cards };
            self.report(Action::Labelling(looked));
        }
    }
}

impl Shared {
    /// Whether this computer can label a scan: the model looked for in the folder chosen and
    /// then beside the program, then the runtime beside the program and then in the folder
    /// chosen. With it, the model if it can, and the cards. Where nothing was found and no
    /// folder was chosen, the words say so and what to do; where a folder was chosen, they
    /// say what is wrong with it.
    fn looked(
        &self,
        models: Option<&Path>,
        runtime: Option<&Path>,
    ) -> (Ready, Option<Arc<Model>>, Vec<Card>) {
        let labeller = self.labelling.labeller.as_ref();
        let cards = labeller.cards();
        let beside = std::env::current_exe().ok();
        let beside = beside.as_deref().and_then(Path::parent);
        let chosen = models.map(Path::to_path_buf).into_iter();
        let in_models = chosen.chain(beside.map(|folder| folder.join(MODELS)));
        let model = match first(in_models, read_model, &ModelError::NotFound) {
            Ok(model) => model,
            Err(ModelError::NotFound) if models.is_none() => return not_ready(NO_MODEL, cards),
            Err(why) => return not_ready(&why.to_string(), cards),
        };
        let beside = beside.map(|folder| folder.join(RUNTIME)).into_iter();
        let in_runtime = beside.chain(runtime.map(Path::to_path_buf));
        let load = |folder: &Path| labeller.load_runtime(folder);
        match first(in_runtime, load, &RuntimeError::NotFound) {
            Ok(()) => (Ready::Yes, Some(Arc::new(model)), cards),
            Err(RuntimeError::NotFound) if runtime.is_none() => not_ready(NO_RUNTIME, cards),
            Err(why) => not_ready(&why.to_string(), cards),
        }
    }
}

/// What a look found when this computer cannot label a scan, and why: the words, then where
/// to read what goes where.
fn not_ready(why: &str, cards: Vec<Card>) -> (Ready, Option<Arc<Model>>, Vec<Card>) {
    (Ready::No(format!("{why} {WHERE}")), None, cards)
}

/// What the first of these folders gives that gives anything. Failing that, the first thing
/// found wrong that is more than `missing`, or `missing` itself.
fn first<T, E: PartialEq + Clone>(
    folders: impl Iterator<Item = PathBuf>,
    from: impl Fn(&Path) -> Result<T, E>,
    missing: &E,
) -> Result<T, E> {
    let mut wrong = None;
    for folder in folders {
        match from(&folder) {
            Ok(found) => return Ok(found),
            Err(why) if why != *missing => {
                wrong.get_or_insert(why);
            }
            Err(_) => {}
        }
    }
    Err(wrong.unwrap_or_else(|| missing.clone()))
}
