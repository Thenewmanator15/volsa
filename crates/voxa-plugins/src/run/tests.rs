//! One run of a plugin from its grid to its labels, through a stand-in helper.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use voxa_core::Volume;
use voxa_models::{Device, RunError, SUPPORTED, Stop};

use super::{Installed, PluginRunError, Stage, installed, run_plugin};
use crate::describe::tests::example;
use crate::grid::tests::volume_of;
use crate::testkit::helper::{InProcessStarter, Threshold};
use crate::testkit::onnx::{conv_network, image_input};
use crate::testkit::plugin_folder;
use crate::{Fits, PluginError, check_folder};

/// A scan of 12 x 6 x 6 voxels of 1 mm, bright (0.9) in a block of 4 x 2 x 2 and dim elsewhere.
fn scan() -> Volume {
    volume_of([12, 6, 6], |col, row, slice| {
        let bright = (4..8).contains(&col) && (2..4).contains(&row) && (2..4).contains(&slice);
        if bright { 0.9 } else { 0.1 }
    })
}

/// The example, its brightness as stored, its axes as Voxa's own, fitted as `fit` says.
fn described(fit: Value) -> Value {
    let mut description = example();
    description
        .as_object_mut()
        .expect("an object")
        .remove("files");
    description["input"]["axes"] = json!("LPS");
    description["input"]["fit"] = fit;
    description["input"]["scaling"] = json!({ "kind": "none" });
    description
}

/// A plugin installed from a folder made for the test, kept with its folder.
fn plugin(fit: Value) -> (Installed, tempfile::TempDir) {
    let network = conv_network(&image_input(), &[0.0, 1.0], &[0.0, -0.5]);
    let folder = plugin_folder(&described(fit), &[("strip.onnx", &network)]);
    let offered = check_folder(folder.path(), SUPPORTED).expect("offered");
    let installed = Installed {
        folder: folder.path().to_owned(),
        description: offered.description,
        sum: offered.sum,
        wiring: offered.wiring,
        measured: None,
    };
    (installed, folder)
}

/// A place on the processor with a cap.
fn processor() -> Fits {
    Fits {
        device: Device::Processor,
        cap: 1 << 30,
    }
}

/// Runs the plugin on the scan with the starter given, the stages said kept.
fn run(
    plugin: &Installed,
    starter: &InProcessStarter,
    stop: &Stop,
    stages: &mut Vec<Stage>,
) -> Result<super::Ran, PluginRunError> {
    let mut said = |stage: Stage| stages.push(stage);
    run_plugin(
        plugin,
        &scan(),
        processor(),
        Path::new("runtime"),
        starter,
        stop,
        &mut said,
    )
}

/// How many voxels a label grid labels.
fn labelled(ran: &super::Ran) -> usize {
    ran.grid.ids.iter().filter(|id| **id != 0).count()
}

#[test]
fn a_whole_box_run_gives_labels_and_the_box_note_when_the_scan_reaches_past_it() {
    let (plugin, _folder) = plugin(json!({ "whole": [8, 8, 8], "outside": "lowest" }));
    let starter = InProcessStarter::default();
    let ran = run(&plugin, &starter, &Stop::new(), &mut Vec::new()).expect("ran");
    assert_eq!(labelled(&ran), 16);
    assert_eq!(ran.grid.dims, [8, 6, 6]);
    let note =
        "The plugin's box of 8 × 8 × 8 mm left out part of the scan, up to 2 mm beyond the box.";
    assert_eq!(ran.notes, [note]);
    assert_eq!(ran.table.title, "Example skull strip");
    assert_eq!(*starter.caps.lock().expect("caps"), [Some(1 << 30)]);
}

#[test]
fn a_block_run_gives_the_same_labels_and_the_blocks_note() {
    let fit = json!({ "blocks": { "size": [4, 4, 4], "step": [2, 2, 2] }, "outside": "zero" });
    let (plugin, _folder) = plugin(fit);
    let ran = run(
        &plugin,
        &InProcessStarter::default(),
        &Stop::new(),
        &mut Vec::new(),
    )
    .expect("ran");
    assert_eq!(labelled(&ran), 16);
    assert_eq!(ran.grid.dims, [12, 6, 6]);
    let note = "Run in blocks: near the edges between blocks, labels may differ from the model's \
                own program.";
    assert_eq!(ran.notes, [note]);
}

#[test]
fn stages_are_said_in_order_with_the_block_count() {
    let fit = json!({ "blocks": { "size": [4, 4, 4], "step": [4, 4, 4] }, "outside": "zero" });
    let (plugin, _folder) = plugin(fit);
    let mut stages = Vec::new();
    run(
        &plugin,
        &InProcessStarter::default(),
        &Stop::new(),
        &mut stages,
    )
    .expect("ran");
    // Starts 0, 4, 8 along the first axis and 0, 2 along the others: 12 blocks.
    let mut expected = vec![Stage::Preparing];
    expected.extend((1..=12).map(|block| Stage::Running { block, of: 12 }));
    expected.push(Stage::Finishing);
    assert_eq!(stages, expected);
}

#[test]
fn stop_ends_the_run_as_stopped() {
    let fit = json!({ "blocks": { "size": [4, 4, 4], "step": [2, 2, 2] }, "outside": "zero" });
    let (plugin, _folder) = plugin(fit);
    let starter = InProcessStarter {
        opener: Threshold {
            slow: true,
            ..Threshold::default()
        },
        ..InProcessStarter::default()
    };
    let stop = Stop::new();
    let watched = stop.clone();
    let began = Instant::now();
    let mut said = |stage: Stage| {
        if stage == (Stage::Running { block: 2, of: 20 }) {
            watched.stop();
        }
    };
    let outcome = run_plugin(
        &plugin,
        &scan(),
        processor(),
        Path::new("runtime"),
        &starter,
        &stop,
        &mut said,
    );
    assert_eq!(
        outcome.expect_err("stopped"),
        PluginRunError::Run(RunError::Stopped)
    );
    assert!(began.elapsed() < Duration::from_secs(3));
}

#[test]
fn a_place_not_measured_is_not_run() {
    let (plugin, _folder) = plugin(json!({ "whole": [8, 8, 8], "outside": "lowest" }));
    let unmeasured = Fits {
        device: Device::Processor,
        cap: 0,
    };
    let starter = InProcessStarter::default();
    let mut said = |_| {};
    let refused = run_plugin(
        &plugin,
        &scan(),
        unmeasured,
        Path::new("runtime"),
        &starter,
        &Stop::new(),
        &mut said,
    );
    assert!(matches!(
        refused,
        Err(PluginRunError::Run(RunError::Failed(_)))
    ));
    assert!(starter.caps.lock().expect("caps").is_empty());
}

#[test]
fn a_plugin_whose_folder_name_is_not_its_sum_is_not_installed() {
    let (_plugin, folder) = plugin(json!({ "whole": [8, 8, 8], "outside": "lowest" }));
    let refused = installed(folder.path(), SUPPORTED).expect_err("not installed");
    assert_eq!(refused, PluginError::Changed);
    let words = "This plugin's files have changed since it was added. Remove it and add it again.";
    assert_eq!(refused.to_string(), words);
}

#[test]
fn a_scan_cut_into_more_blocks_than_voxa_runs_is_refused_before_any_is_made() {
    let fit = json!({ "blocks": { "size": [2, 2, 2], "step": [1, 1, 1] }, "outside": "zero" });
    let (plugin, _folder) = plugin(fit);
    let large = volume_of([40, 40, 40], |_, _, _| 0.5);
    let refused = run_plugin(
        &plugin,
        &large,
        processor(),
        Path::new("runtime"),
        &InProcessStarter::default(),
        &Stop::new(),
        &mut |_| {},
    )
    .expect_err("refused");
    assert!(
        matches!(refused, PluginRunError::TooManyBlocks(59_319)),
        "{refused:?}"
    );
    assert_eq!(
        refused.to_string(),
        "This plugin would cut this scan into 59319 blocks, more than the 10000 Voxa runs."
    );
}
