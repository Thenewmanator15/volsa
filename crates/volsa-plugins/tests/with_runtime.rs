//! Real runs through the plugin helper and ONNX Runtime, of a network the tests write
//! themselves: one 1 x 1 x 1 convolution whose second channel is the value less a half, so that
//! the labels are known exactly (a voxel is labelled where its value is above a half).
//!
//! Every test here is ignored unless asked for, and needs `VOLSA_RUNTIME` to name the folder
//! that holds the runtime; a test says so and fails when it is missing. `cargo xtask
//! check-models` runs them one at a time.
#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};
use std::process::{ChildStdin, ChildStdout};

use volsa_models::{CardKind, Helper, RunError, cards, start_helper};
use volsa_plugins::helper::{Client, HelperError, OpenJob, Where};
use volsa_plugins::testkit::onnx::{conv_network, image_input};
use volsa_plugins::testkit::sum;
use volsa_plugins::{BlockPlan, Gives, LabelMap, LabelOutput, Layout, Name, Region, Side};

/// The folder of the runtime, from `VOLSA_RUNTIME`.
fn runtime() -> PathBuf {
    let folder = std::env::var_os("VOLSA_RUNTIME").filter(|folder| !folder.is_empty());
    PathBuf::from(folder.expect("VOLSA_RUNTIME must name the folder that holds the runtime"))
}

/// A cube of `side` voxels, bright (0.9) in a ball off its centre and dim (0.1) elsewhere.
fn values(side: usize) -> Vec<f32> {
    let mut values = Vec::with_capacity(side * side * side);
    let centre = [side as f64 * 0.4, side as f64 * 0.55, side as f64 * 0.5];
    let radius = side as f64 / 4.0;
    for a in 0..side {
        for b in 0..side {
            for c in 0..side {
                let away: f64 = [a, b, c]
                    .iter()
                    .zip(centre)
                    .map(|(at, centre)| (*at as f64 - centre).powi(2))
                    .sum();
                values.push(if away.sqrt() < radius { 0.9 } else { 0.1 });
            }
        }
    }
    values
}

/// The job for the convolution in `folder`, run on `device` in blocks of `block`.
fn job(folder: &Path, device: Where, block: [usize; 3]) -> OpenJob {
    let network = folder.join("conv.onnx");
    let bytes = conv_network(&image_input(), &[0.0, 1.0], &[0.0, -0.5]);
    std::fs::write(&network, &bytes).expect("written");
    let [x, y, z] = block;
    OpenJob {
        runtime: runtime(),
        network,
        sha256: sum(&bytes),
        device,
        free: vec![
            ("x".to_owned(), x),
            ("y".to_owned(), y),
            ("z".to_owned(), z),
        ],
        output: None,
        layout: Layout::ChannelsFirst,
        block,
        finish: LabelOutput {
            gives: Gives::Scores {
                channels: vec![0, 1],
            },
            from: None,
            keep_largest_piece: false,
            names: vec![Name {
                id: 1,
                name: "Bright".to_owned(),
                side: Side::Midline,
                group: String::new(),
                colour: None,
            }],
        },
    }
}

/// A helper started for real, capped at `cap` bytes if given, and Volsa's end of its pipes.
fn helper(cap: Option<u64>) -> (Helper, Client<ChildStdout, ChildStdin>) {
    let program = Path::new(env!("CARGO_BIN_EXE_volsa-plugin-helper"));
    let mut helper = start_helper(program, &[], cap).expect("the helper started");
    let (input, output) = helper.take_pipes().expect("its pipes");
    (helper, Client::new(output, input))
}

/// The labels of a cube of `side` voxels run whole on `device`.
fn whole(device: Where, side: usize) -> Vec<u16> {
    let folder = tempfile::tempdir().expect("a folder");
    let (_helper, mut client) = helper(None);
    client
        .open(&job(folder.path(), device, [side; 3]))
        .expect("opened");
    let labels = client.block(&values(side)).expect("labels");
    client.finish().expect("finished");
    labels
}

/// What the labels must be: 1 where a value is above a half.
fn expected(side: usize) -> Vec<u16> {
    values(side)
        .iter()
        .map(|value| u16::from(*value > 0.5))
        .collect()
}

#[test]
#[ignore = "needs VOLSA_RUNTIME"]
fn a_conv_network_labels_a_whole_box_on_the_processor() {
    assert_eq!(whole(Where::Processor, 32), expected(32));
}

#[test]
#[ignore = "needs VOLSA_RUNTIME"]
fn the_same_network_in_blocks_gives_the_same_labels() {
    let side = 32;
    let all = values(side);
    let plan = BlockPlan {
        grid: [side; 3],
        size: [16; 3],
        step: [8; 3],
    };
    let folder = tempfile::tempdir().expect("a folder");
    let (_helper, mut client) = helper(None);
    client
        .open(&job(folder.path(), Where::Processor, plan.size))
        .expect("opened");
    let region = Region {
        from: [0; 3],
        size: plan.grid,
    };
    let mut map = LabelMap::new(region).expect("a map");
    for start in plan.starts() {
        let mut block = Vec::with_capacity(16 * 16 * 16);
        for a in start[0]..start[0] + 16 {
            for b in start[1]..start[1] + 16 {
                block.extend_from_slice(&all[(a * side + b) * side + start[2]..][..16]);
            }
        }
        let labels = client.block(&block).expect("labels");
        let at = start.map(|side| isize::try_from(side).expect("small"));
        map.put(at, plan.size, plan.owned(start), &labels);
    }
    client.finish().expect("finished");
    assert_eq!(map.ids(), expected(side).as_slice());
}

#[test]
#[ignore = "needs VOLSA_RUNTIME"]
fn each_discrete_card_gives_the_processors_labels() {
    let discrete: Vec<_> = cards()
        .into_iter()
        .filter(|card| card.kind == CardKind::Discrete)
        .collect();
    if discrete.is_empty() {
        println!("this computer has no discrete graphics card; nothing to check");
        return;
    }
    for card in discrete {
        let place = Where::Card {
            id: card.id.0,
            name: card.name.clone(),
        };
        assert_eq!(whole(place, 32), expected(32), "{}", card.name);
    }
}

#[test]
#[ignore = "needs VOLSA_RUNTIME"]
fn a_cap_below_the_networks_need_is_refused_not_obeyed() {
    let folder = tempfile::tempdir().expect("a folder");
    let (mut helper, mut client) = helper(Some(64 << 20));
    // A 256 cube's input alone is 64 MiB, and its two channels of scores twice that.
    let side = 256;
    let refused = client
        .open(&job(folder.path(), Where::Processor, [side; 3]))
        .and_then(|()| client.block(&values(side)).map(|_| ()));
    let refused = refused.expect_err("a run past its cap fails");
    println!("refused with: {refused:?}");
    assert_eq!(refused, HelperError::Run(RunError::NoRoom));
    helper.stop();
}
