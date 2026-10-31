//! What the state does with plugins: the wrong modality refused, a contrast warned of, runs
//! that take turns with labelling, and a result opened as labels.

use std::path::PathBuf;

use voxa_models::{RunsOn, SUPPORTED};
use voxa_plugins::Stage;
use voxa_plugins::testkit::example_folder;

use crate::action::{Action, Effect};
use crate::labels::LabelsStatus;
use crate::plugins::{PluginAction, PluginEffect, PluginEntry};
use crate::reducer::reduce;
use crate::state::{AppState, NoticeKind};
use crate::testing::{THICK_SCAN, id, thick_scan_ready};

/// A state with the thick scan shown, described as `description`, of `modality`, and the
/// example plugin listed.
fn ready(modality: &str, description: &str) -> AppState {
    let mut state = thick_scan_ready();
    let index = state.index.as_mut().expect("an index");
    let series = index
        .series
        .iter_mut()
        .find(|series| series.info.id == id(THICK_SCAN))
        .expect("listed");
    series.info.modality = modality.to_owned();
    series.info.description = description.to_owned();
    let folder = example_folder("1.0");
    let offered = voxa_plugins::check_folder(folder.path(), SUPPORTED).expect("offered");
    let entry = PluginEntry::of_offered(&offered, "the processor");
    let look = state.plugins.looks;
    reduce(
        &mut state,
        plugins(PluginAction::Looked {
            look,
            entries: vec![entry],
        }),
    );
    state
}

/// A plugin action as an action.
fn plugins(action: PluginAction) -> Action {
    Action::Plugins(action)
}

/// Asks to run the example plugin.
fn run(state: &mut AppState) -> Vec<Effect> {
    let id = "example.skull-strip".to_owned();
    reduce(state, plugins(PluginAction::Run { id, held: 0 }))
}

/// The plugin run an effect asks for, if it asks for one.
fn asked_run(effects: &[Effect]) -> Option<(u64, Option<String>)> {
    effects.iter().find_map(|effect| match effect {
        Effect::Plugins(PluginEffect::Run { run, stamp, .. }) => Some((*run, stamp.clone())),
        _ => None,
    })
}

#[test]
fn a_ct_scan_posts_the_refusal_and_runs_nothing() {
    let mut state = ready("CT", "Head");
    let effects = run(&mut state);
    assert_eq!(asked_run(&effects), None);
    let notice = state.notices.last().expect("a notice");
    assert_eq!(notice.kind, NoticeKind::Error);
    assert_eq!(notice.text, "This plugin is for MR scans; this scan is CT.");
    assert!(state.plugins.running.is_none());
}

#[test]
fn a_mismatch_warns_and_runs_only_on_confirmation_with_the_stamp() {
    let mut state = ready("MR", "Ax T2 TSE");
    let effects = run(&mut state);
    assert_eq!(asked_run(&effects), None);
    let warning = state.plugins.warning.clone().expect("a warning");
    assert_eq!(
        warning.warning,
        "This plugin expects T1; this scan's description says T2."
    );
    let effects = reduce(&mut state, plugins(PluginAction::ConfirmRun));
    let stamp = "Run after a warning: this plugin expects T1; this scan's description says T2.";
    assert_eq!(asked_run(&effects), Some((1, Some(stamp.to_owned()))));
    assert!(state.plugins.warning.is_none());
    let running = state.plugins.running.as_ref().expect("running");
    assert_eq!(running.stage, Stage::Preparing);
}

#[test]
fn cancel_drops_the_warning_and_runs_nothing() {
    let mut state = ready("MR", "Ax T2 TSE");
    run(&mut state);
    let effects = reduce(&mut state, plugins(PluginAction::Cancel));
    assert_eq!(asked_run(&effects), None);
    assert!(state.plugins.warning.is_none() && state.plugins.running.is_none());
}

#[test]
fn a_matched_scan_runs_with_its_note() {
    let mut state = ready("MR", "Sag MPRAGE");
    let effects = run(&mut state);
    let note = "The scan's description says 3D T1, as this plugin expects.".to_owned();
    assert_eq!(asked_run(&effects), Some((1, Some(note))));
    let Some(Effect::Plugins(PluginEffect::Run { runs_on, .. })) = effects
        .iter()
        .find(|effect| matches!(effect, Effect::Plugins(PluginEffect::Run { .. })))
    else {
        panic!("a run");
    };
    assert_eq!(*runs_on, RunsOn::Automatic);
}

#[test]
fn a_plugin_run_stops_a_labelling_run_and_the_other_way() {
    let mut state = ready("MR", "T1");
    state.labelling.running = Some(voxa_segment::Stage::Preparing);
    let effects = run(&mut state);
    assert!(effects.contains(&Effect::StopLabelling));
    assert!(state.labelling.running.is_none());
    assert!(state.plugins.running.is_some());
    let stopped = crate::reducer::plugins::stop(&mut state);
    assert_eq!(stopped, Some(Effect::Plugins(PluginEffect::Stop)));
    assert!(state.plugins.running.is_none());
}

#[test]
fn an_old_runs_stage_is_ignored() {
    let mut state = ready("MR", "T1");
    run(&mut state);
    let old = Stage::Running { block: 1, of: 2 };
    reduce(
        &mut state,
        plugins(PluginAction::Stage { run: 7, stage: old }),
    );
    assert_eq!(
        state.plugins.running.as_ref().expect("running").stage,
        Stage::Preparing
    );
    let new = Stage::Running { block: 1, of: 2 };
    reduce(
        &mut state,
        plugins(PluginAction::Stage { run: 1, stage: new }),
    );
    assert_eq!(state.plugins.running.as_ref().expect("running").stage, new);
}

#[test]
fn a_failed_run_says_why() {
    let mut state = ready("MR", "T1");
    run(&mut state);
    let why = "The plugin stopped without finishing. Voxa is unaffected.".to_owned();
    reduce(
        &mut state,
        plugins(PluginAction::Failed {
            run: 1,
            why: why.clone(),
        }),
    );
    assert!(state.plugins.running.is_none());
    assert_eq!(state.notices.last().expect("a notice").text, why);
}

#[test]
fn a_made_result_is_opened_as_labels() {
    let mut state = ready("MR", "T1");
    run(&mut state);
    let (labels, table) = (PathBuf::from("labels.nii.gz"), PathBuf::from("labels.json"));
    let made = PluginAction::Made {
        run: 1,
        labels: labels.clone(),
        table: table.clone(),
    };
    let effects = reduce(&mut state, plugins(made));
    assert!(state.plugins.running.is_none());
    assert_eq!(state.labels.status, LabelsStatus::Reading);
    assert!(effects.contains(&Effect::ReadLabels {
        labels,
        table: Some(table)
    }));
}

#[test]
fn remove_asks_before_it_removes() {
    let mut state = ready("MR", "T1");
    let id = "example.skull-strip".to_owned();
    let effects = reduce(&mut state, plugins(PluginAction::Remove(id.clone())));
    assert_eq!(effects, []);
    assert_eq!(state.plugins.removing.as_deref(), Some(id.as_str()));
    let effects = reduce(&mut state, plugins(PluginAction::ConfirmRemove));
    assert_eq!(effects, [Effect::Plugins(PluginEffect::Remove(id.clone()))]);
    reduce(&mut state, plugins(PluginAction::Removed(Ok(id))));
    assert_eq!(state.plugins.entries, []);
}

#[test]
fn an_offered_folder_shows_its_card_and_add_sends_it_on() {
    let mut state = ready("MR", "T1");
    let folder = PathBuf::from("a folder");
    let effects = reduce(&mut state, plugins(PluginAction::Offer(folder.clone())));
    assert_eq!(effects, [Effect::Plugins(PluginEffect::Offer(folder))]);
    assert!(state.plugins.busy.is_some());
    let source = example_folder("2.0");
    let offered = voxa_plugins::check_folder(source.path(), SUPPORTED).expect("offered");
    let card = crate::plugins::OfferCard::of(&offered, &state.plugins.entries);
    assert_eq!(card.replaces.as_deref(), Some("1.0"));
    reduce(&mut state, plugins(PluginAction::Offered(Ok(card))));
    assert!(state.plugins.busy.is_none() && state.plugins.offered.is_some());
    let effects = reduce(&mut state, plugins(PluginAction::Add));
    assert_eq!(
        effects,
        [Effect::Plugins(PluginEffect::Add { runtime: None })]
    );
    assert!(state.plugins.offered.is_none());
}
