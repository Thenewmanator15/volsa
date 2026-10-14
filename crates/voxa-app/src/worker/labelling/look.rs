//! Looking at whether this computer can label a scan: the model found and checked, the
//! runtime loaded, and the cards listed. What the newest look found is kept and told; what an
//! older look found is neither, so that it cannot take the place of the newer.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use voxa_models::{Card, Model, ModelError, RuntimeError, read_model};

use super::Kit;
use crate::action::Action;
use crate::labelling::{LabellingAction, Ready};
use crate::worker::{Shared, Workers, lock, shielded};

/// Said after anything that keeps this computer from labelling a scan.
const WHERE: &str = "docs/models.md says what goes where.";
/// The folders looked in beside the program.
const MODELS: &str = "models";
const RUNTIME: &str = "runtime";
/// What is said when a look stops part-way.
const PART_WAY: &str = "Voxa could not finish checking the model and the runtime.";

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
    /// Look, on a worker, for the model and the runtime, the model first, and list the cards.
    /// Reading a model folder and loading the runtime each take seconds.
    pub(super) fn look(&self, look: u64, models: Option<PathBuf>, runtime: Option<PathBuf>) {
        self.shared.labelling.asked_to_look(look);
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            let mut found = (Ready::No(format!("{PART_WAY} {WHERE}")), None, Vec::new());
            shielded(|| found = shared.looked(models.as_deref(), runtime.as_deref()));
            let (ready, model, cards) = found;
            if shared.labelling.found(look, model) {
                let looked = LabellingAction::LabellingLooked { look, ready, cards };
                shared.report(Action::Labelling(looked));
            }
        });
    }
}

impl Shared {
    /// Whether this computer can label a scan: the model looked for in the folder chosen and
    /// then beside the program, then the runtime beside the program and then in the folder
    /// chosen. With it, the model if it can, and the cards.
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
            Err(why) => return (Ready::No(format!("{why} {WHERE}")), None, cards),
        };
        let beside = beside.map(|folder| folder.join(RUNTIME)).into_iter();
        let in_runtime = beside.chain(runtime.map(Path::to_path_buf));
        let load = |folder: &Path| labeller.load_runtime(folder);
        match first(in_runtime, load, &RuntimeError::NotFound) {
            Ok(()) => (Ready::Yes, Some(Arc::new(model)), cards),
            Err(why) => (Ready::No(format!("{why} {WHERE}")), None, cards),
        }
    }
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
