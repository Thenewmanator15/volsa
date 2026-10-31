//! The plugins' work on the workers, with the labelling lab's stand-ins for the computer and a
//! stand-in helper served in this process: a folder offered and added and measured, a run kept
//! for its series and found again, and the ways a run fails.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;
use tempfile::TempDir;
use voxa_models::RunsOn;
use voxa_plugins::testkit::helper::{InProcessStarter, Threshold};
use voxa_plugins::testkit::onnx::{conv_network, image_input};
use voxa_plugins::testkit::{example, plugin_folder};

use super::lab::Lab;
use super::stand_ins::StandIn;
use crate::action::{Action, Effect};
use crate::plugins::{PluginAction, PluginEffect};
use crate::testing::{THICK_SCAN, id};

/// A GiB.
const GIB: u64 = 1 << 30;
/// What the stand-in helper says it held at most.
pub(super) const PEAK: u64 = 64 << 20;
/// The example plugin's identity.
const ID: &str = "example.skull-strip";

/// A folder that passes for a runtime's: it holds a file of the library's name.
pub(super) fn runtime() -> TempDir {
    let folder = tempfile::tempdir().expect("a folder");
    let library = if cfg!(windows) {
        "onnxruntime.dll"
    } else {
        "libonnxruntime.so"
    };
    fs::write(folder.path().join(library), b"made-up runtime").expect("written");
    folder
}

/// The example plugin, its brightness scaled from the scan's lowest to its highest value, so
/// that the made-up head's brighter half is labelled.
pub(super) fn source() -> TempDir {
    source_of("1.0")
}

/// The same plugin at `version`.
fn source_of(version: &str) -> TempDir {
    let mut description = example();
    description["version"] = json!(version);
    description["input"]["scaling"] =
        json!({ "kind": "percentiles", "low": 0, "high": 100, "over": "scan" });
    let network = conv_network(&image_input(), &[0.0, 1.0], &[0.0, -0.5]);
    plugin_folder(&description, &[("strip.onnx", &network)])
}

/// The lab, keeping settings, its helpers served in this process by `opener`.
pub(super) fn lab(opener: Threshold) -> Lab {
    let lab = Lab::keeping(StandIn::with(8 * GIB, 32 * GIB));
    let starter = InProcessStarter {
        opener,
        peak: PEAK,
        ..InProcessStarter::default()
    };
    lab.workers.set_plugin_starter(Arc::new(starter));
    lab
}

/// What the workers say about plugins, until `end` holds of something said.
pub(super) fn until(lab: &Lab, end: impl Fn(&PluginAction) -> bool) -> Vec<PluginAction> {
    let mut said = Vec::new();
    loop {
        if let Action::Plugins(action) = lab.next() {
            let done = end(&action);
            said.push(action);
            if done {
                return said;
            }
        }
    }
}

/// Offers the example and adds it, with the runtime in `runtime`: what was said at the end.
fn add(lab: &Lab, source: &Path, runtime: &Path) -> PluginAction {
    lab.workers.run(
        Effect::Plugins(PluginEffect::Offer(source.to_owned())),
        None,
    );
    let offered = until(lab, |said| matches!(said, PluginAction::Offered(_)));
    assert!(
        matches!(offered.last(), Some(PluginAction::Offered(Ok(_)))),
        "{offered:?}"
    );
    let asked = PluginEffect::Add {
        runtime: Some(runtime.to_owned()),
    };
    lab.workers.run(Effect::Plugins(asked), None);
    let said = until(lab, |said| matches!(said, PluginAction::Added(_)));
    said.last().cloned().expect("an end")
}

/// Runs the example on the 30-slice series with `stamp`: what was said at the end.
fn run(lab: &Lab, runtime: &Path, stamp: &str) -> PluginAction {
    let asked = PluginEffect::Run {
        run: 1,
        id: ID.to_owned(),
        series: id(THICK_SCAN),
        stamp: Some(stamp.to_owned()),
        runs_on: RunsOn::Automatic,
        held: 0,
        drawing_on: None,
        runtime: Some(runtime.to_owned()),
    };
    lab.workers.run(Effect::Plugins(asked), None);
    let said = until(lab, |said| {
        matches!(
            said,
            PluginAction::Made { .. } | PluginAction::Failed { .. }
        )
    });
    said.last().cloned().expect("an end")
}

#[test]
fn offering_a_good_folder_reports_its_card() {
    let lab = lab(Threshold::default());
    let source = source();
    lab.workers.run(
        Effect::Plugins(PluginEffect::Offer(source.path().to_owned())),
        None,
    );
    let said = until(&lab, |said| matches!(said, PluginAction::Offered(_)));
    let Some(PluginAction::Offered(Ok(card))) = said.last() else {
        panic!("a card: {said:?}");
    };
    assert_eq!(card.entry.name, "Example skull strip");
    assert_eq!(card.replaces, None);
    assert_eq!(card.settings.len(), 7);
}

#[test]
fn adding_measures_and_reports_where_it_runs() {
    let lab = lab(Threshold::default());
    let (source, runtime) = (source(), runtime());
    let PluginAction::Added(Ok(entry)) = add(&lab, source.path(), runtime.path()) else {
        panic!("added");
    };
    assert_eq!(entry.runs_on, "the processor and Made-up card A");
    let plugins = lab.settings().join("plugins");
    let copies: Vec<PathBuf> = fs::read_dir(&plugins)
        .expect("the plugins folder")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(copies.len(), 1);
    assert!(copies[0].join("measured.json").is_file());
}

#[test]
fn a_plugin_whose_processor_measurement_fails_is_not_kept() {
    let lab = lab(Threshold {
        refuse: true,
        ..Threshold::default()
    });
    let (source, runtime) = (source(), runtime());
    let PluginAction::Added(Err(why)) = add(&lab, source.path(), runtime.path()) else {
        panic!("refused");
    };
    assert!(why.starts_with("The plugin was not added"), "{why}");
    let plugins = lab.settings().join("plugins");
    assert_eq!(
        fs::read_dir(&plugins).expect("the plugins folder").count(),
        0
    );
}

#[test]
fn a_run_reports_labels_made_by_the_plugin_with_added_by_you_and_the_stamp() {
    let lab = lab(Threshold::default());
    let (source, runtime) = (source(), runtime());
    add(&lab, source.path(), runtime.path());
    let said = run(&lab, runtime.path(), "A stamp.");
    let PluginAction::Made { run, labels, table } = said else {
        panic!("made: {said:?}");
    };
    assert_eq!(run, 1);
    assert!(labels.is_file());
    let text = fs::read_to_string(&table).expect("the table");
    let read = voxa_labels::read_table("labels.json", &text).expect("a table");
    let made = read.made.expect("who made it");
    assert_eq!(made.notes.first().map(String::as_str), Some("A stamp."));
    let added = made.added.expect("added by the person");
    assert_eq!(added.id, ID);
    assert_eq!(made.device, "Made-up card A");
}

#[test]
fn a_run_is_kept_for_its_series_and_plugin_and_found_again() {
    let lab = lab(Threshold::default());
    let (source, runtime) = (source(), runtime());
    add(&lab, source.path(), runtime.path());
    let said = run(&lab, runtime.path(), "A stamp.");
    let PluginAction::Made { labels, .. } = said else {
        panic!("made: {said:?}");
    };
    let asked = PluginEffect::ShowKept {
        id: ID.to_owned(),
        series: id(THICK_SCAN),
    };
    lab.workers.run(Effect::Plugins(asked), None);
    loop {
        if let Action::OpenLabels {
            labels: found,
            table,
        } = lab.next()
        {
            assert_eq!(found, labels);
            assert!(table.is_some());
            break;
        }
    }
}

#[test]
fn a_helper_that_ends_early_says_so_and_voxa_goes_on() {
    let lab = lab(Threshold::default());
    let (source, runtime) = (source(), runtime());
    add(&lab, source.path(), runtime.path());
    let dying = InProcessStarter {
        opener: Threshold {
            dies: true,
            ..Threshold::default()
        },
        peak: PEAK,
        ..InProcessStarter::default()
    };
    lab.workers.set_plugin_starter(Arc::new(dying));
    let PluginAction::Failed { why, .. } = run(&lab, runtime.path(), "A stamp.") else {
        panic!("failed");
    };
    assert_eq!(
        why,
        "The plugin stopped without finishing. Voxa is unaffected."
    );
}

#[test]
fn a_run_falls_to_the_processor_when_the_card_has_no_room() {
    let lab = lab(Threshold::default());
    let (source, runtime) = (source(), runtime());
    let PluginAction::Added(Ok(entry)) = add(&lab, source.path(), runtime.path()) else {
        panic!("added");
    };
    assert_eq!(entry.runs_on, "the processor and Made-up card A");
    let full = InProcessStarter {
        opener: Threshold {
            no_room_on_cards: true,
            ..Threshold::default()
        },
        peak: PEAK,
        ..InProcessStarter::default()
    };
    lab.workers.set_plugin_starter(Arc::new(full));
    let said = run(&lab, runtime.path(), "A stamp.");
    let PluginAction::Made { table, .. } = said else {
        panic!("made: {said:?}");
    };
    let text = fs::read_to_string(&table).expect("the table");
    let made = voxa_labels::read_table("labels.json", &text)
        .expect("a table")
        .made
        .expect("who made it");
    assert_eq!(made.device, "the processor");
}

#[test]
fn a_card_the_system_lists_twice_is_measured_once() {
    let twice = StandIn::of(
        vec![super::stand_ins::card_a(), super::stand_ins::card_a()],
        vec![Some(8 * GIB), Some(8 * GIB)],
        32 * GIB,
    );
    let lab = Lab::keeping(twice);
    let starter = InProcessStarter {
        peak: PEAK,
        ..InProcessStarter::default()
    };
    lab.workers.set_plugin_starter(Arc::new(starter));
    let (source, runtime) = (source(), runtime());
    let PluginAction::Added(Ok(entry)) = add(&lab, source.path(), runtime.path()) else {
        panic!("added");
    };
    assert_eq!(entry.runs_on, "the processor and Made-up card A");
    let plugins = lab.settings().join("plugins");
    let copy = fs::read_dir(&plugins)
        .expect("the plugins folder")
        .next()
        .expect("one copy")
        .expect("an entry")
        .path();
    let measured = voxa_plugins::read_measured(&copy).expect("measured.json");
    assert_eq!(measured.cards.len(), 1, "{measured:?}");
}

/// Asks for the result kept for the 30-slice series by the example plugin: the labels opened,
/// or the sentence said instead.
fn shown(lab: &Lab) -> Result<PathBuf, String> {
    let asked = PluginEffect::ShowKept {
        id: ID.to_owned(),
        series: id(THICK_SCAN),
    };
    lab.workers.run(Effect::Plugins(asked), None);
    loop {
        match lab.next() {
            Action::OpenLabels { labels, .. } => return Ok(labels),
            Action::Post(notice) => return Err(notice.text),
            _ => {}
        }
    }
}

#[test]
fn a_result_is_found_by_the_plugins_identity_after_a_new_version_and_after_removing_it() {
    let lab = lab(Threshold::default());
    let (source, runtime) = (source(), runtime());
    add(&lab, source.path(), runtime.path());
    let PluginAction::Made { labels, .. } = run(&lab, runtime.path(), "A stamp.") else {
        panic!("made");
    };
    let newer = source_of("2.0");
    let PluginAction::Added(Ok(entry)) = add(&lab, newer.path(), runtime.path()) else {
        panic!("version 2 added");
    };
    assert_eq!(entry.version, "2.0");
    assert_eq!(shown(&lab), Ok(labels.clone()), "after a new version");
    lab.workers
        .run(Effect::Plugins(PluginEffect::Remove(ID.to_owned())), None);
    until(&lab, |said| matches!(said, PluginAction::Removed(_)));
    assert_eq!(shown(&lab), Ok(labels), "after it is removed");
}
