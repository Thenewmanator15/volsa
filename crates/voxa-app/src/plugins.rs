//! Plugins as the state keeps them: those added, a folder offered and its card, a warning
//! waiting for the person, and the run on. The Tools tab of the interface redesign draws
//! [`ToolEntry`]s; until it lands, the Parts tab does.

use std::path::PathBuf;

use voxa_core::SeriesId;
use voxa_models::RunsOn;
use voxa_plugins::{Accepts, Contrasts, Description, Installed, Offered, Output, Stage};

mod store;
mod words;

#[allow(
    unused_imports,
    reason = "the workers that use them come with the next commit"
)]
pub(crate) use store::{add, folder, list, remove, same_id};
pub use words::settings_lines;

#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod tests;

/// One plugin added, as its entry in the Tools tab shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginEntry {
    /// Its identity.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// Who made it, in its maker's words.
    pub made_by: String,
    /// Where it came from: text, never a link.
    pub source: String,
    /// Its licence.
    pub licence: String,
    /// What it does, in one sentence.
    pub about: String,
    /// The limits its maker wants shown.
    pub limits: Vec<String>,
    /// Whether its files passed their check when last looked at; the sentence if not.
    pub checked: Result<(), String>,
    /// What scans it accepts.
    pub accepts: Accepts,
    /// What scans it accepts, as a phrase.
    pub accepts_words: String,
    /// What it gives back, as a phrase.
    pub returns: String,
    /// Where it can run, as a phrase.
    pub runs_on: String,
}

impl PluginEntry {
    /// The entry of a description.
    fn of_description(description: &Description, runs_on: &str) -> Self {
        Self {
            id: description.id.clone(),
            name: description.name.clone(),
            version: description.version.clone(),
            made_by: description.made_by.clone(),
            source: description.source.clone(),
            licence: description.licence.clone(),
            about: description.about.clone(),
            limits: description.limits.clone(),
            checked: Ok(()),
            accepts: description.accepts.clone(),
            accepts_words: words::accepts(&description.accepts),
            returns: words::returns(&description.output),
            runs_on: runs_on.to_owned(),
        }
    }

    /// The entry of a folder offered.
    pub fn of_offered(offered: &Offered, runs_on: &str) -> Self {
        Self::of_description(&offered.description, runs_on)
    }

    /// The entry of a plugin added.
    pub fn of_installed(installed: &Installed, runs_on: &str) -> Self {
        Self::of_description(&installed.description, runs_on)
    }

    /// The entry of a copy that failed its check, named by its folder, saying why.
    pub fn broken(folder: &str, why: String) -> Self {
        let accepts = Accepts {
            modality: Vec::new(),
            contrast: Contrasts::Any,
            body_part: String::new(),
            thickest_mm: None,
        };
        Self {
            id: folder.to_owned(),
            name: folder.to_owned(),
            version: String::new(),
            made_by: String::new(),
            source: String::new(),
            licence: String::new(),
            about: String::new(),
            limits: Vec::new(),
            checked: Err(why),
            accepts,
            accepts_words: String::new(),
            returns: String::new(),
            runs_on: String::new(),
        }
    }
}

/// The card a folder offered is shown on before it is added.
#[derive(Debug, Clone, PartialEq)]
pub struct OfferCard {
    /// What the plugin says of itself.
    pub entry: PluginEntry,
    /// Every preparation setting, in plain words, a line each.
    pub settings: Vec<String>,
    /// The version of a plugin of the same identity already added, which it would replace.
    pub replaces: Option<String>,
}

impl OfferCard {
    /// The card of a folder offered, beside the plugins already added.
    pub fn of(offered: &Offered, added: &[PluginEntry]) -> Self {
        let description = &offered.description;
        let settings = match &description.output {
            Output::Labels(output) => settings_lines(&description.input, output),
            _ => Vec::new(),
        };
        let replaces = added
            .iter()
            .find(|entry| entry.id == description.id)
            .map(|entry| entry.version.clone());
        Self {
            entry: PluginEntry::of_offered(offered, "measured when it is added"),
            settings,
            replaces,
        }
    }
}

/// A warning shown before a run, waiting for the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// The plugin to run.
    pub id: String,
    /// What the person is told.
    pub warning: String,
    /// What the result carries if it runs.
    pub stamp: String,
    /// What Voxa holds of the card it draws on, as the run was asked for.
    pub held: u64,
}

/// The run on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    /// The plugin running.
    pub id: String,
    /// Its number.
    pub run: u64,
    /// How far it has got.
    pub stage: Stage,
    /// The series it runs on.
    pub series: SeriesId,
}

/// Plugins in the state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plugins {
    /// The plugins added, in the order found.
    pub entries: Vec<PluginEntry>,
    /// A folder offered, checked, waiting for Add or Cancel.
    pub offered: Option<OfferCard>,
    /// A warning waiting for Run anyway or Cancel.
    pub warning: Option<Warning>,
    /// A plugin to remove, waiting for the person to confirm.
    pub removing: Option<String>,
    /// What the workers are doing for plugins, as a sentence, while they are.
    pub busy: Option<String>,
    /// The run on.
    pub running: Option<Running>,
    /// How many runs have been asked for: the number of the newest.
    pub runs: u64,
    /// How many looks at the plugins folder have been asked for: the number of the newest.
    pub looks: u64,
}

/// Everything that can happen to plugins.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginAction {
    /// Look again at the plugins added.
    Look,
    /// A worker has listed the plugins added.
    Looked {
        /// Which look.
        look: u64,
        /// Their entries.
        entries: Vec<PluginEntry>,
    },
    /// The person chose a folder to add.
    Offer(PathBuf),
    /// A worker checked the folder: its card, or why it is refused.
    Offered(Result<OfferCard, String>),
    /// Add the folder offered.
    Add,
    /// Close the card, the warning or the question, doing nothing.
    Cancel,
    /// A worker added a plugin and measured it, or says why it did not.
    Added(Result<PluginEntry, String>),
    /// What a worker is doing for plugins now.
    Busy(String),
    /// Remove a plugin: asked first.
    Remove(String),
    /// Remove the plugin asked about.
    ConfirmRemove,
    /// A worker removed a plugin, or says why it did not.
    Removed(Result<String, String>),
    /// Run a plugin on the scan in the scene.
    Run {
        /// The plugin.
        id: String,
        /// What Voxa holds of the card it draws on.
        held: u64,
    },
    /// Run anyway, after a warning.
    ConfirmRun,
    /// A run has reached a stage.
    Stage {
        /// Which run.
        run: u64,
        /// The stage.
        stage: Stage,
    },
    /// A run failed.
    Failed {
        /// Which run.
        run: u64,
        /// Why, in a sentence.
        why: String,
    },
    /// A run made labels, kept in these files.
    Made {
        /// Which run.
        run: u64,
        /// The label file.
        labels: PathBuf,
        /// Its table.
        table: PathBuf,
    },
    /// Stop the run on.
    Stop,
    /// Open the result kept for the scan in the scene by this plugin.
    ShowKept(String),
    /// Delete the result kept for the scan in the scene by this plugin.
    DeleteKept(String),
}

/// What the workers are asked to do for plugins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginEffect {
    /// List the plugins added.
    Look {
        /// Which look.
        look: u64,
    },
    /// Check a folder offered.
    Offer(PathBuf),
    /// Add the folder last offered, and measure it.
    Add,
    /// Remove a plugin.
    Remove(String),
    /// Run a plugin.
    Run {
        /// Its number.
        run: u64,
        /// The plugin.
        id: String,
        /// The series to run it on.
        series: SeriesId,
        /// The note or stamp the result carries first.
        stamp: Option<String>,
        /// Where the person wants it to go.
        runs_on: RunsOn,
        /// What Voxa holds of the card it draws on.
        held: u64,
        /// The name of that card.
        drawing_on: Option<String>,
        /// The runtime's folder the person chose, if one.
        runtime: Option<PathBuf>,
    },
    /// Stop the run on.
    Stop,
    /// Open the result kept for a series by a plugin.
    ShowKept {
        /// The plugin.
        id: String,
        /// The series.
        series: SeriesId,
    },
    /// Delete the result kept for a series by a plugin.
    DeleteKept {
        /// The plugin.
        id: String,
        /// The series.
        series: SeriesId,
    },
}

/// One entry of the Tools tab's "Added by you", with the state of its plugin and the names of
/// its actions in the list of actions.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolEntry {
    /// The plugin.
    pub entry: PluginEntry,
    /// What it is doing, or why it cannot run, as a sentence.
    pub state: String,
    /// Whether it is running now.
    pub running: bool,
    /// The action that runs it: `plugin.<id>.run`, and so on for the others.
    pub run: String,
    /// The action that stops it.
    pub stop: String,
    /// The action that opens its kept result.
    pub show: String,
    /// The action that deletes its kept result.
    pub delete: String,
    /// The action that removes it.
    pub remove: String,
}

/// The action that offers a folder: `plugins.add`.
pub const ADD_ACTION: &str = "plugins.add";

/// The entries of the Tools tab's "Added by you".
pub fn tool_entries(plugins: &Plugins) -> Vec<ToolEntry> {
    plugins
        .entries
        .iter()
        .map(|entry| {
            let running = plugins.running.as_ref().filter(|run| run.id == entry.id);
            let state = match (running, &entry.checked) {
                (Some(run), _) => words::stage(run.stage),
                (None, Err(why)) => why.clone(),
                (None, Ok(())) => "Ready.".to_owned(),
            };
            let action = |what: &str| format!("plugin.{}.{what}", entry.id);
            ToolEntry {
                entry: entry.clone(),
                state,
                running: running.is_some(),
                run: action("run"),
                stop: action("stop"),
                show: action("show"),
                delete: action("delete"),
                remove: action("remove"),
            }
        })
        .collect()
}
