//! A real plugin, made by someone else, run on the made-up head that the labelling checks use:
//! added to a folder of its own as Voxa adds one, measured, and run through the helper program
//! and ONNX Runtime on the processor and on each discrete card. The brain it marks is held
//! against the made-up head's brain as `SynthSeg` labels it: it must cover nearly all of the
//! brain (every label but 0 and 24), and agree with the brain and the fluid around it (every
//! label but 0), as a skull strip keeps that fluid. Its agreement with the brain alone is
//! printed too.
//!
//! Ignored unless asked for. It needs `VOXA_TEST_PLUGIN` (the plugin's folder: no plugin comes
//! with Voxa, so without it the test says so and does nothing), `VOXA_MODELS` (a model folder,
//! whose `check` folder holds the head) and `VOXA_RUNTIME` (the runtime's folder).
#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value;
use voxa_core::{Affine, Point, Volume};
use voxa_labels::LabelGrid;
use voxa_models::{CardKind, Device, SUPPORTED, Stop, cards};
use voxa_plugins::{
    Fit, Fits, Grid, Installed, Over, ProcessStarter, Scale, Scaling, check_folder, gather,
    installed, measure, measuring_job, outside_value, run_plugin, sample, voxels_of,
};

/// The least Dice score of the plugin's mask against the head's brain and the fluid around it.
const LEAST_DICE: f64 = 0.85;
/// The least share of the head's brain, without that fluid, that the plugin's mask covers.
const LEAST_COVER: f64 = 0.90;
/// `SynthSeg`'s label of the fluid around the brain, which a skull strip keeps.
const CSF: u16 = 24;

/// The made-up head's brain, a flag per voxel: without the fluid around it, and with it.
struct Brain {
    alone: Vec<bool>,
    with_fluid: Vec<bool>,
}

/// A folder named by an environment variable, if it is set.
fn named(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from)
}

/// The plugin added to `into` as Voxa adds one: its description and listed files copied into a
/// folder named by the first 16 digits of the description's sum, then checked again.
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

/// The made-up head, as Voxa holds one, and the brain `SynthSeg` finds in it.
fn head(models: &Path) -> (Volume, Brain) {
    let check = models.join("check");
    let case: Value = serde_json::from_slice(&fs::read(check.join("case.json")).expect("case"))
        .expect("case.json is JSON");
    let side = |place: usize| case["volume"]["dims"][place].as_u64().expect("a side");
    let dims = [0, 1, 2].map(|place| u32::try_from(side(place)).expect("a side"));
    // case.json places the volume as NIfTI does, x to the right and y to the front; Voxa's x
    // runs to the left and its y to the back.
    let number = |row: usize, place: usize| case["volume"]["affine"][row][place].as_f64();
    let number = |row, place| number(row, place).expect("case.json places the volume");
    let column = |place| [-number(0, place), -number(1, place), number(2, place)];
    let placed = Affine::from_columns(column(0), column(1), column(2), column(3));
    let bytes = fs::read(check.join("volume.f32")).expect("volume.f32");
    let values = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|&four| f32::from_le_bytes(four));
    let volume = Volume::new(dims, values.collect(), placed).expect("a volume");
    let labels = fs::read(check.join("standard.u16")).expect("standard.u16");
    let labels: Vec<u16> = labels
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&two| u16::from_le_bytes(two))
        .collect();
    let brain = Brain {
        alone: labels
            .iter()
            .map(|&label| label != 0 && label != CSF)
            .collect(),
        with_fluid: labels.iter().map(|&label| label != 0).collect(),
    };
    (volume, brain)
}

/// Whether the plugin labelled each voxel of the head, read from its grid where the voxel lies.
fn marked(volume: &Volume, grid: &LabelGrid) -> Vec<bool> {
    let to_grid = grid
        .placement
        .inverse()
        .expect("a grid that can be inverted");
    let to_patient = volume.voxel_to_patient();
    let [columns, rows, slices] = volume.dims();
    let [width, height, depth] = grid.dims.map(i64::from);
    let mut marked = Vec::with_capacity(volume.values().len());
    for slice in 0..slices {
        for row in 0..rows {
            for column in 0..columns {
                let at = Point::new(f64::from(column), f64::from(row), f64::from(slice));
                let on = to_grid.apply(to_patient.apply(at));
                #[expect(clippy::cast_possible_truncation, reason = "rounded voxel numbers")]
                let [x, y, z] = [on.x, on.y, on.z].map(|place| place.round() as i64);
                let inside = (0..width).contains(&x) && (0..height).contains(&y);
                let inside = inside && (0..depth).contains(&z);
                let index = usize::try_from(x + width * (y + height * z)).unwrap_or(0);
                marked.push(inside && grid.ids[index] != 0);
            }
        }
    }
    marked
}

/// The Dice score of two masks, and the share of `truth` that `found` covers.
fn agreement(found: &[bool], truth: &[bool]) -> (f64, f64) {
    let count = |mask: &[bool]| mask.iter().filter(|&&on| on).count();
    let both = found.iter().zip(truth).filter(|&(&a, &b)| a && b).count();
    #[expect(clippy::cast_precision_loss, reason = "counts of voxels")]
    let (both, found, truth) = (both as f64, count(found) as f64, count(truth) as f64);
    (2.0 * both / (found + truth), both / truth)
}

/// Measures the plugin on `device`, runs it there, and gives the run's seconds, the helper's
/// measured peak and the plugin's labels.
fn run_on(
    plugin: &Installed,
    device: Device,
    volume: &Volume,
    runtime: &Path,
) -> (f64, u64, LabelGrid) {
    let starter = ProcessStarter {
        program: PathBuf::from(env!("CARGO_BIN_EXE_voxa-plugin-helper")),
    };
    let job = measuring_job(plugin, device, runtime).expect("a job to measure");
    let (ordinary, _) = measure(&starter, &job, None).expect("measured");
    let place = Fits {
        device,
        cap: ordinary.saturating_add(ordinary / 4),
    };
    let clock = Instant::now();
    let ran = run_plugin(
        plugin,
        volume,
        place,
        runtime,
        &starter,
        &Stop::new(),
        &mut |_| {},
    )
    .expect("the plugin runs");
    (clock.elapsed().as_secs_f64(), ordinary, ran.grid)
}

#[test]
#[ignore = "needs VOXA_TEST_PLUGIN, VOXA_MODELS and VOXA_RUNTIME"]
fn a_plugin_named_by_voxa_test_plugin_masks_the_made_up_head() {
    let Some(plugin_folder) = named("VOXA_TEST_PLUGIN") else {
        println!("VOXA_TEST_PLUGIN is not set: no plugin to check, so nothing was run");
        return;
    };
    let models = named("VOXA_MODELS").expect("VOXA_MODELS must name a model folder");
    let runtime = named("VOXA_RUNTIME").expect("VOXA_RUNTIME must name the runtime's folder");
    let added_to = tempfile::tempdir().expect("a folder for plugins");
    let plugin = added(&plugin_folder, added_to.path());
    let (volume, brain) = head(&models);
    let discrete = cards()
        .into_iter()
        .filter(|card| card.kind == CardKind::Discrete);
    let places = std::iter::once((Device::Processor, "the processor".to_owned()))
        .chain(discrete.map(|card| (Device::Card(card.id), card.name)));
    for (device, name) in places {
        let (seconds, peak, grid) = run_on(&plugin, device, &volume, &runtime);
        let mask = marked(&volume, &grid);
        let (dice, _) = agreement(&mask, &brain.with_fluid);
        let (alone, cover) = agreement(&mask, &brain.alone);
        println!(
            "{name}: Dice {dice:.3} with the brain and its fluid, {alone:.3} with the brain \
             alone, covers {cover:.3} of the brain, run {seconds:.1} s, helper's measured peak \
             {} MiB",
            peak >> 20
        );
        assert!(
            dice >= LEAST_DICE,
            "{name}: Dice {dice:.3} is below {LEAST_DICE}"
        );
        assert!(
            cover >= LEAST_COVER,
            "{name}: covers {cover:.3}, below {LEAST_COVER}"
        );
    }
}

#[test]
#[ignore = "needs VOXA_TEST_PLUGIN and VOXA_MODELS; a measurement: run it in a release build"]
fn preparing_the_plugins_whole_box_from_the_made_up_head_takes_at_most_2_s() {
    let Some(plugin_folder) = named("VOXA_TEST_PLUGIN") else {
        println!("VOXA_TEST_PLUGIN is not set: no plugin to prepare a scan for");
        return;
    };
    let models = named("VOXA_MODELS").expect("VOXA_MODELS must name a model folder");
    let added_to = tempfile::tempdir().expect("a folder for plugins");
    let plugin = added(&plugin_folder, added_to.path());
    let (volume, _) = head(&models);
    let input = &plugin.description.input;
    let Fit::Whole(size) = input.fit else {
        panic!("this budget is for a plugin of one whole box");
    };
    // The steps a run takes before its box goes to the helper, as `run_plugin` takes them:
    // the grid, the box, the statistics and the scaling, then the box sampled and scaled.
    let clock = Instant::now();
    let grid = Grid::of(&volume, input);
    let region = grid.whole_box(size);
    let over = match input.scaling {
        Scaling::Percentiles {
            over: Over::Grid, ..
        }
        | Scaling::ZScore {
            over: Over::Grid, ..
        } => Some(region),
        _ => None,
    };
    let stats = gather(&volume, &grid, over, voxels_of(&input.scaling)).expect("statistics");
    let scale = Scale::new(&input.scaling, &stats).expect("a scale");
    let outside = outside_value(input.outside, &stats);
    let mut values = sample(&volume, &grid, region, outside).expect("the box sampled");
    scale.apply(&mut values);
    let seconds = clock.elapsed().as_secs_f64();
    println!(
        "preparing a box of {:?} on the processor: {seconds:.2} s ({} values)",
        size,
        values.len()
    );
    assert!(seconds <= 2.0, "{seconds:.2} s is over the budget of 2 s");
}
