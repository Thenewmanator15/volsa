//! Actions about plugins: the plugins added, a folder offered, a run asked for (refused,
//! warned of, or started), what a run reports, and its result opened as labels. A plugin run
//! and a labelling run take turns: starting one stops the other.

use voxa_plugins::{Decision, Stage, decide};

use super::{labelling, labels};
use crate::action::Effect;
use crate::plugins::{PluginAction, PluginEffect, Plugins, Running, Warning};
use crate::state::{AppState, Notice, NoticeKind};

/// The key of the notice about plugins: the latest replaces the one before.
const PLUGIN: &str = "plugin";
/// What is said when a run is stopped.
const STOPPED: &str = "The plugin was stopped.";
/// What is said when there is no scan with a 3D view to run a plugin on.
const NO_SCAN: &str = "Open a scan that has a 3D view to run a plugin on it.";

/// Apply an action about plugins.
pub(super) fn reduce(state: &mut AppState, action: PluginAction) -> Vec<Effect> {
    match action {
        about_a_run @ (PluginAction::Run { .. }
        | PluginAction::ConfirmRun
        | PluginAction::Stage { .. }
        | PluginAction::Failed { .. }
        | PluginAction::Made { .. }
        | PluginAction::Stop
        | PluginAction::ShowKept(_)
        | PluginAction::DeleteKept(_)) => runs(state, about_a_run),
        about_the_folder => folder(state, about_the_folder),
    }
}

/// Apply an action about the plugins added, a folder offered, or a removal.
fn folder(state: &mut AppState, action: PluginAction) -> Vec<Effect> {
    let plugins = &mut state.plugins;
    match action {
        PluginAction::Look => {
            plugins.looks += 1;
            let look = plugins.looks;
            return vec![effect(PluginEffect::Look { look })];
        }
        PluginAction::Looked { look, entries } if look == plugins.looks => {
            plugins.entries = entries;
        }
        PluginAction::Offer(folder) => {
            plugins.busy = Some("Checking the folder.".to_owned());
            return vec![effect(PluginEffect::Offer(folder))];
        }
        PluginAction::Offered(card) => {
            plugins.busy = None;
            match card {
                Ok(card) => plugins.offered = Some(card),
                Err(why) => say(state, NoticeKind::Error, why),
            }
        }
        PluginAction::Add if plugins.offered.take().is_some() => {
            plugins.busy = Some("Adding the plugin and measuring what it needs.".to_owned());
            let runtime = state.labelling.runtime.clone();
            return vec![effect(PluginEffect::Add { runtime })];
        }
        PluginAction::Cancel => {
            plugins.offered = None;
            plugins.warning = None;
            plugins.removing = None;
        }
        PluginAction::Added(added) => added_one(state, added),
        PluginAction::Busy(what) => plugins.busy = Some(what),
        PluginAction::Remove(id) => plugins.removing = Some(id),
        PluginAction::ConfirmRemove => {
            if let Some(id) = plugins.removing.take() {
                return vec![effect(PluginEffect::Remove(id))];
            }
        }
        PluginAction::Removed(Ok(id)) => plugins.entries.retain(|entry| entry.id != id),
        PluginAction::Removed(Err(why)) => say(state, NoticeKind::Error, why),
        // Old news, nothing offered, or an action about a run, which is not this function's.
        _ => {}
    }
    Vec::new()
}

/// Apply an action about a run.
fn runs(state: &mut AppState, action: PluginAction) -> Vec<Effect> {
    let plugins = &mut state.plugins;
    match action {
        PluginAction::Run { id, held } => return asked(state, id, held),
        PluginAction::ConfirmRun => {
            if let Some(warning) = plugins.warning.take() {
                return start(state, warning.id, Some(warning.stamp), warning.held);
            }
        }
        PluginAction::Stage { run, stage } => {
            if let Some(running) = plugins.running.as_mut().filter(|on| on.run == run) {
                running.stage = stage;
            }
        }
        PluginAction::Failed { run, why } if waiting_for(plugins, run) => {
            plugins.running = None;
            say(state, NoticeKind::Error, why);
        }
        PluginAction::Made { run, labels, table } if waiting_for(plugins, run) => {
            let series = plugins.running.take().map(|running| running.series);
            // A result for a series no longer in the scene is old news.
            if series.is_some() && series == state.scene.series {
                return labels::open(state, labels, Some(table));
            }
        }
        PluginAction::Stop => return stop(state).into_iter().collect(),
        PluginAction::ShowKept(id) => return kept(state, id, true),
        PluginAction::DeleteKept(id) => return kept(state, id, false),
        // What an older run says is old news.
        _ => {}
    }
    Vec::new()
}

/// An effect about plugins.
fn effect(effect: PluginEffect) -> Effect {
    Effect::Plugins(effect)
}

/// Posts a notice about plugins in place of the one before.
fn say(state: &mut AppState, kind: NoticeKind, text: String) {
    let notice = Notice {
        kind,
        text,
        details: Vec::new(),
        key: Some(PLUGIN),
    };
    super::post(state, PLUGIN, Some(notice));
}

/// Whether run `run` is the one on.
fn waiting_for(plugins: &Plugins, run: u64) -> bool {
    plugins
        .running
        .as_ref()
        .is_some_and(|running| running.run == run)
}

/// A plugin was added: its entry in place of one of the same identity, or why it was not.
fn added_one(state: &mut AppState, added: Result<crate::plugins::PluginEntry, String>) {
    state.plugins.busy = None;
    match added {
        Ok(entry) => {
            let entries = &mut state.plugins.entries;
            entries.retain(|known| known.id != entry.id);
            entries.push(entry);
        }
        Err(why) => say(state, NoticeKind::Error, why),
    }
}

/// A run asked for: refused, warned of, or started, by the scan in the scene.
fn asked(state: &mut AppState, id: String, held: u64) -> Vec<Effect> {
    let entry = state.plugins.entries.iter().find(|entry| entry.id == id);
    let Some(entry) = entry.cloned() else {
        return Vec::new();
    };
    let info = state
        .scene
        .series
        .clone()
        .filter(|_| crate::planes::grid(state).is_some())
        .and_then(|series| state.series(&series).map(|series| series.info.clone()));
    let Some(info) = info else {
        say(state, NoticeKind::Warning, NO_SCAN.to_owned());
        return Vec::new();
    };
    if let Err(why) = &entry.checked {
        say(state, NoticeKind::Error, why.clone());
        return Vec::new();
    }
    match decide(&info.modality, &info.description, &entry.accepts) {
        Decision::Refuse(why) => {
            say(state, NoticeKind::Error, why);
            Vec::new()
        }
        Decision::Warn { warning, stamp } => {
            state.plugins.warning = Some(Warning {
                id,
                warning,
                stamp,
                held,
            });
            Vec::new()
        }
        Decision::Run { note } => start(state, id, note, held),
    }
}

/// Starts a run: a run of labelling and a plugin run before it are stopped first.
fn start(state: &mut AppState, id: String, stamp: Option<String>, held: u64) -> Vec<Effect> {
    let Some(series) = state.scene.series.clone() else {
        return Vec::new();
    };
    let mut effects: Vec<Effect> = labelling::stop(state).into_iter().collect();
    effects.extend(stop_quietly(&mut state.plugins));
    super::post(state, PLUGIN, None);
    let plugins = &mut state.plugins;
    plugins.runs += 1;
    plugins.running = Some(Running {
        id: id.clone(),
        run: plugins.runs,
        stage: Stage::Preparing,
        series: series.clone(),
    });
    effects.push(effect(PluginEffect::Run {
        run: plugins.runs,
        id,
        series,
        stamp,
        runs_on: state.labelling.runs_on,
        held,
        drawing_on: state.labelling.drawing_on.clone(),
        runtime: state.labelling.runtime.clone(),
    }));
    effects
}

/// Stops the run on, if one is, saying nothing.
fn stop_quietly(plugins: &mut Plugins) -> Option<Effect> {
    plugins.running.take().map(|_| effect(PluginEffect::Stop))
}

/// Stops the run on and says so; with none on, does nothing.
pub(super) fn stop(state: &mut AppState) -> Option<Effect> {
    let stopped = stop_quietly(&mut state.plugins)?;
    say(state, NoticeKind::Info, STOPPED.to_owned());
    Some(stopped)
}

/// Opens, or deletes, the result kept for the scan in the scene by a plugin.
fn kept(state: &mut AppState, id: String, show: bool) -> Vec<Effect> {
    let Some(series) = state.scene.series.clone() else {
        say(state, NoticeKind::Warning, NO_SCAN.to_owned());
        return Vec::new();
    };
    let asked = if show {
        PluginEffect::ShowKept { id, series }
    } else {
        PluginEffect::DeleteKept { id, series }
    };
    vec![effect(asked)]
}

/// What of the plugins outlives the export left as another is opened: the plugins added, a
/// folder offered or a plugin asked about, what the workers are doing for them, and the counts
/// of looks and runs, so that the answer to a look asked for before is still taken. A warning
/// about the scan left goes, and the run, which was stopped.
pub(super) fn emptied(plugins: &Plugins) -> Box<Plugins> {
    Box::new(Plugins {
        warning: None,
        running: None,
        ..plugins.clone()
    })
}
