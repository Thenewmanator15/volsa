//! The plugins the person added, on the workers: listing them, checking a folder offered,
//! adding it and measuring what it needs (the module `add`), removing one, opening or deleting
//! a result kept for a series, and running one (the module `run`). Every plugin network runs in
//! a helper process, started by the kit's starter: Voxa itself, `--plugin-helper`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use voxa_core::SeriesId;
use voxa_models::{SUPPORTED, Stop};
use voxa_plugins::{HelperStarter, Installed, Measured, Offered, ProcessStarter};

use super::{Shared, Workers, lock};
use crate::action::Action;
use crate::kept_labels::{forget, kept};
use crate::plugins::{PluginAction, PluginEffect, PluginEntry, folder, list};
use crate::state::NoticeKind;
use crate::views::series_key;

mod add;
mod run;

/// What is said when there is nowhere to keep plugins.
const NO_SETTINGS: &str =
    "Voxa has no folder for its settings on this computer, so it cannot keep plugins.";
/// What is said when there is no runtime to run a plugin with.
const NO_RUNTIME: &str = "Plugins need ONNX Runtime, as labelling does: put it in a folder called \
                          runtime beside Voxa, or choose its folder under Options.";
/// What is said when a plugin asked for is no longer among those added.
const GONE: &str = "That plugin is no longer among those added.";
/// What is said when no result of a plugin is kept for the scan.
const NONE_KEPT: &str = "No result of this plugin is kept for this scan.";
/// The key of the notices about plugins the workers post.
const KEY: &str = "plugin";
/// The runtime's library, as a runtime folder holds it.
const LIBRARY: &str = if cfg!(windows) {
    "onnxruntime.dll"
} else {
    "libonnxruntime.so"
};

/// What the workers keep for plugins.
pub(super) struct Kit {
    /// What starts a helper.
    starter: Mutex<Arc<dyn HelperStarter>>,
    /// The folder last offered, checked, until it is added or another is offered.
    offered: Mutex<Option<Offered>>,
    /// The stop of the run on, if one is.
    run: Mutex<Option<Stop>>,
}

impl Default for Kit {
    fn default() -> Self {
        let program = std::env::current_exe().unwrap_or_default();
        Self {
            starter: Mutex::new(Arc::new(ProcessStarter { program })),
            offered: Mutex::new(None),
            run: Mutex::new(None),
        }
    }
}

impl Kit {
    /// What starts a helper now.
    fn starter(&self) -> Arc<dyn HelperStarter> {
        Arc::clone(&lock(&self.starter))
    }
}

impl Workers {
    /// Starts plugin helpers with `starter` from now on, in place of Voxa itself: for tests.
    pub fn set_plugin_starter(&self, starter: Arc<dyn HelperStarter>) {
        *lock(&self.shared.plugins.starter) = starter;
    }

    /// Carry out an effect about plugins.
    pub(super) fn plugins(&self, effect: PluginEffect) {
        let shared = Arc::clone(&self.shared);
        match effect {
            PluginEffect::Look { look } => self.spawn(move || shared.look_at_plugins(look)),
            PluginEffect::Offer(folder) => self.spawn(move || shared.offer(&folder)),
            PluginEffect::Add { runtime } => self.spawn(move || shared.add_offered(runtime)),
            PluginEffect::Remove(id) => self.spawn(move || shared.remove_plugin(&id)),
            PluginEffect::Stop => {
                if let Some(stop) = lock(&self.shared.plugins.run).take() {
                    stop.stop();
                }
            }
            PluginEffect::ShowKept { id, series } => {
                self.spawn(move || shared.kept_result(&id, &series, true));
            }
            PluginEffect::DeleteKept { id, series } => {
                self.spawn(move || shared.kept_result(&id, &series, false));
            }
            run @ PluginEffect::Run { .. } => self.run_plugin(run),
        }
    }
}

/// Where the plugins it uses can run, as a phrase: from what they were measured on.
pub(super) fn runs_on_words(measured: Option<&Measured>) -> String {
    let Some(measured) = measured else {
        return "not measured yet".to_owned();
    };
    let mut places: Vec<String> = Vec::new();
    if measured.processor.is_some() {
        places.push("the processor".to_owned());
    }
    places.extend(measured.cards.iter().map(|card| card.name.clone()));
    match places.as_slice() {
        [] => "nowhere yet".to_owned(),
        [only] => format!("{only} only"),
        _ => places.join(" and "),
    }
}

/// The folder of the runtime: a `runtime` folder beside Voxa, then the one the person chose,
/// whichever holds the runtime's library.
pub(super) fn runtime_folder(chosen: Option<PathBuf>) -> Option<PathBuf> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|folder| folder.join("runtime")));
    beside
        .into_iter()
        .chain(chosen)
        .find(|folder| folder.join(LIBRARY).is_file())
}

/// The runtime's library in a runtime folder.
pub(super) fn library(runtime: &Path) -> PathBuf {
    runtime.join(LIBRARY)
}

/// The key a plugin's result for a series is kept under: the series' own key, `p`, and the
/// first 16 digits of the plugin's sum, letters and digits only as a kept key must be.
fn kept_key(series: &SeriesId, plugin: &Installed) -> String {
    let named = plugin.sum.get(..16).unwrap_or_default();
    format!("{}p{named}", series_key(series))
}

impl Shared {
    /// Tell the state something about plugins.
    fn tell_plugins(&self, action: PluginAction) {
        self.report(Action::Plugins(action));
    }

    /// The folder plugins are kept in, if there is a settings folder.
    fn plugins_folder(&self) -> Option<PathBuf> {
        self.settings.as_deref().map(folder)
    }

    /// The plugins added, each checked; the broken ones by their folder's name.
    fn added(&self) -> Vec<Result<Installed, (String, voxa_plugins::PluginError)>> {
        self.plugins_folder()
            .map(|folder| list(&folder, SUPPORTED))
            .unwrap_or_default()
    }

    /// The plugin added with this identity, if it is there and passes its check.
    fn plugin(&self, id: &str) -> Option<Installed> {
        self.added()
            .into_iter()
            .filter_map(Result::ok)
            .find(|plugin| plugin.description.id == id)
    }

    /// List the plugins added, for the state.
    fn look_at_plugins(&self, look: u64) {
        let entries = self
            .added()
            .into_iter()
            .map(|plugin| match plugin {
                Ok(plugin) => {
                    PluginEntry::of_installed(&plugin, &runs_on_words(plugin.measured.as_ref()))
                }
                Err((name, why)) => PluginEntry::broken(&name, why.to_string()),
            })
            .collect();
        self.tell_plugins(PluginAction::Looked { look, entries });
    }

    /// Open, or delete, the result a plugin made for a series.
    fn kept_result(&self, id: &str, series: &SeriesId, show: bool) {
        let (Some(settings), Some(plugin)) = (self.settings.as_deref(), self.plugin(id)) else {
            return self.say(NoticeKind::Warning, NONE_KEPT, KEY);
        };
        let key = kept_key(series, &plugin);
        if !show {
            if forget(settings, &key).is_err() {
                self.say(
                    NoticeKind::Warning,
                    "The kept result could not be deleted.",
                    KEY,
                );
            }
            return;
        }
        match kept(settings, &key) {
            Some((labels, table)) => self.report(Action::OpenLabels {
                labels,
                table: Some(table),
            }),
            None => self.say(NoticeKind::Info, NONE_KEPT, KEY),
        }
    }
}
