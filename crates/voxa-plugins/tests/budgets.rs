//! Two budgets of the plugin design (section 15) that need no model: checking a plugin whose
//! network is 100 MB, and how long a helper takes to start and load the runtime. Each prints
//! what it measured and fails when the budget is missed.
//!
//! Ignored unless asked for; measure in a release build:
//! `cargo test --release -p voxa-plugins --features testkit --test budgets -- --ignored
//! --nocapture`. The helper's start-up needs `VOXA_RUNTIME`, and starts
//! `VOXA_HELPER_PROGRAM` (`voxa.exe`, which takes `--plugin-helper`) when that is set, the
//! test helper program otherwise.
#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use voxa_models::{Device, SUPPORTED};
use voxa_plugins::testkit::onnx::{
    DimSpec, GraphSpec, InitSpec, NetSpec, NodeSpec, floats, image_input, network_bytes,
};
use voxa_plugins::testkit::{example, example_folder, plugin_folder};
use voxa_plugins::{Installed, ProcessStarter, check_folder, installed, measure, measuring_job};

/// How many 32-bit numbers make the network 100 MB.
const BALLAST: usize = 25_000_000;

/// The example's one convolution, with a tensor of 100 MB beside it that nothing reads.
fn network_of_100_mb() -> Vec<u8> {
    let input = image_input();
    let mut output_dims = input.dims.clone();
    output_dims[1] = DimSpec::Fixed(2);
    let tensor = |name: &str, dims: Vec<i64>, numbers: Vec<f32>| InitSpec {
        name: name.to_owned(),
        dims,
        floats: numbers,
        external: None,
    };
    let conv = NodeSpec {
        op: "Conv".to_owned(),
        inputs: vec!["image".to_owned(), "w".to_owned(), "b".to_owned()],
        outputs: vec!["scores".to_owned()],
        attributes: vec![voxa_plugins::testkit::onnx::AttrSpec::Ints(
            "kernel_shape".to_owned(),
            vec![1, 1, 1],
        )],
        ..NodeSpec::default()
    };
    let ballast = i64::try_from(BALLAST).expect("a size");
    let graph = GraphSpec {
        nodes: vec![conv],
        inputs: vec![input],
        outputs: vec![floats("scores", output_dims)],
        initializers: vec![
            tensor("w", vec![2, 1, 1, 1, 1], vec![0.0, 1.0]),
            tensor("b", vec![2], vec![0.0, -0.5]),
            tensor("ballast", vec![ballast], vec![0.25; BALLAST]),
        ],
    };
    network_bytes(&NetSpec {
        ir_version: 8,
        opsets: vec![(String::new(), 17)],
        graph,
        functions: 0,
    })
}

/// A folder named by an environment variable, if it is set.
fn named(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The plugin in `from` added to `into` as Voxa adds one.
fn added(from: &Path, into: &Path) -> Installed {
    let offered = check_folder(from, SUPPORTED).expect("the plugin passes every check");
    let copy = into.join(&offered.sum[..16]);
    fs::create_dir(&copy).expect("a folder for the copy");
    fs::copy(from.join("plugin.json"), copy.join("plugin.json")).expect("plugin.json copied");
    for file in &offered.description.files {
        fs::copy(from.join(&file.name), copy.join(&file.name)).expect("a listed file copied");
    }
    installed(&copy, SUPPORTED).expect("the copy passes every check")
}

#[test]
#[ignore = "a measurement: run it in a release build"]
fn checking_a_plugin_of_100_mb_takes_at_most_2_s() {
    let network = network_of_100_mb();
    assert!(network.len() >= 100_000_000, "{} bytes", network.len());
    let folder = plugin_folder(&example(), &[("strip.onnx", &network)]);
    let clock = Instant::now();
    check_folder(folder.path(), SUPPORTED).expect("the plugin passes every check");
    let seconds = clock.elapsed().as_secs_f64();
    println!(
        "checking a plugin of {} MB: {seconds:.2} s",
        network.len() / 1_000_000
    );
    assert!(seconds <= 2.0, "{seconds:.2} s is over the budget of 2 s");
}

#[test]
#[ignore = "needs VOXA_RUNTIME; a measurement: run it in a release build"]
fn a_helper_starts_and_loads_the_runtime_within_a_second_and_a_half() {
    let runtime = named("VOXA_RUNTIME").expect("VOXA_RUNTIME must name the runtime's folder");
    let program = named("VOXA_HELPER_PROGRAM")
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_voxa-plugin-helper")));
    let source = example_folder("1.0");
    let added_to = tempfile::tempdir().expect("a folder for plugins");
    let plugin = added(source.path(), added_to.path());
    let job = measuring_job(&plugin, Device::Processor, &runtime).expect("a job");
    let starter = ProcessStarter { program };
    let mut slowest = 0.0_f64;
    for attempt in 1..=3 {
        // Started, the runtime loaded, a network of one convolution opened and given one
        // block of 8 x 8 x 8: all but the start and the load is a few milliseconds.
        let clock = Instant::now();
        measure(&starter, &job, None).expect("the helper answers");
        let seconds = clock.elapsed().as_secs_f64();
        println!("helper started, runtime loaded, run {attempt}: {seconds:.2} s");
        slowest = slowest.max(seconds);
    }
    assert!(slowest <= 1.5, "{slowest:.2} s is over the budget of 1.5 s");
}
