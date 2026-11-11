//! The whole labelling path with the real networks, held against what `SynthSeg`'s own Python
//! made of the same made-up head: the `check` folder of a model folder, which
//! `tools/synthseg-to-onnx/check.py` writes and describes.
//!
//! Every test here is ignored unless it is asked for. Each needs `VOXA_MODELS` to name a model
//! folder, and those that run a network need `VOXA_RUNTIME` to name the folder that holds the
//! runtime; a test says which is missing and fails. `cargo xtask check-models` runs them one
//! at a time, each in a process of its own. A test for a kind of graphics card this computer
//! has not got says so and passes. Each whole run on the processor or on a discrete card is
//! also held to a bound on the ordinary memory it may ask the system for.

use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use std::time::Instant;
use std::{env, fs};

use serde_json::Value;
use voxa_core::{Affine, Point, Volume};
use voxa_labels::LabelGrid;
use voxa_models::{
    CardKind, Cut, Device, Model, ModelSide, Numbers, Runtime, Tensor, Wanted, budget, read_model,
};
use voxa_segment::testing::{Agreement, Bound, OnDevice, cards_of, measured, quickest};
use voxa_segment::{Stage, find, label, prepare, tidy};

#[path = "with_models/by_size.rs"]
mod by_size;

/// Any model and device but the standard model on the processor, which is held to every
/// voxel: at least this share of voxels as Python has them, in percent.
const SAME_PERCENT: f64 = 99.99;
/// And at least this Dice for every structure.
const DICE: f64 = 0.999;
/// How far a score may lie from Python's.
const SCORES: f64 = 0.001;
/// How far the image of a volume at 1 mm may lie from Python's, at any voxel.
const IMAGE: f32 = 0.000_01;
/// The same for a volume that had to be resampled.
const RESAMPLED_IMAGE: f32 = 0.000_1;
/// A structure of fewer voxels than this has no middle to speak of.
const ENOUGH: u32 = 100;
/// What this process holds on a card rises by at least this much while a model runs there:
/// 1 GiB. The smallest network takes 2.1 GiB of a card and the scores network 8.1, and
/// before a run the process holds nothing there. A run that went to another card would
/// leave this one where it was.
const CARD_ROSE: u64 = 1 << 30;

/// The standard model, as the check folder has its answer: without the areas of the cortex.
const STANDARD: Wanted = Wanted {
    robust: false,
    cortex: false,
};
/// The robust model, as the check folder has its answer: with the areas of the cortex.
const ROBUST: Wanted = Wanted {
    robust: true,
    cortex: true,
};

/// The variable that names the model folder, and what it names.
const MODELS: (&str, &str) = ("VOXA_MODELS", "a model folder");
/// The same for the networks' runtime.
const RUNTIME: (&str, &str) = ("VOXA_RUNTIME", "the folder that holds the runtime");

/// The folder a variable names. Without it a test that was asked for cannot run, and says so.
fn folder((variable, what): (&str, &str)) -> PathBuf {
    match env::var_os(variable) {
        Some(named) => named.into(),
        None => panic!("This test needs {variable} to name {what}, and it is not set."),
    }
}

/// The `check` folder: a made-up head, and what `SynthSeg`'s own Python made of it.
struct Check {
    folder: PathBuf,
    /// `case.json`: the sizes, where the volumes lie, and Python's scores.
    case: Value,
}

static CHECK: LazyLock<Check> = LazyLock::new(|| {
    let folder = folder(MODELS).join("check");
    let case = fs::read_to_string(folder.join("case.json")).expect("the check folder's case.json");
    let case = serde_json::from_str(&case).expect("case.json is JSON");
    Check { folder, case }
});

/// The runtime loaded and the model folder read, once for every test that runs a network.
static REAL: LazyLock<(Runtime, Model)> = LazyLock::new(|| {
    let runtime = Runtime::load(&folder(RUNTIME)).expect("the runtime loads");
    let model = read_model(&folder(MODELS)).expect("the model folder reads");
    (runtime, model)
});

/// Held by every test that runs a network, so that no two want their gigabytes at once.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn turn() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Check {
    fn bytes(&self, file: &str) -> Vec<u8> {
        fs::read(self.folder.join(file)).unwrap_or_else(|_| panic!("the check folder's {file}"))
    }

    /// A file of 32-bit floating point numbers.
    fn floats(&self, file: &str) -> Vec<f32> {
        let bytes = self.bytes(file);
        let fours = bytes.as_chunks::<4>().0.iter();
        fours.map(|&four| f32::from_le_bytes(four)).collect()
    }

    /// A file of Python's labels, on the volume's grid, columns fastest.
    fn labels(&self, file: &str) -> Vec<u32> {
        let bytes = self.bytes(file);
        let twos = bytes.as_chunks::<2>().0.iter();
        twos.map(|&two| u16::from_le_bytes(two).into()).collect()
    }

    /// Python's scores for a model.
    fn scores(&self, model: &str) -> Vec<f64> {
        let listed = self.case[model]["scores"].as_array();
        let listed = listed.unwrap_or_else(|| panic!("case.json has no scores of {model}"));
        listed.iter().filter_map(Value::as_f64).collect()
    }

    /// The sides `case.json` gives for a volume or an image.
    fn sides<T: TryFrom<u64>>(&self, of: &str) -> [T; 3] {
        let side = |place: usize| self.case[of]["dims"][place].as_u64();
        let side = |place| side(place).and_then(|side| T::try_from(side).ok());
        [0, 1, 2].map(|place| side(place).unwrap_or_else(|| panic!("case.json sizes {of}")))
    }

    /// A volume of the check folder, as Voxa holds one. `case.json` places it as NIfTI does,
    /// with x to the patient's right and y to the front. Voxa's x runs to the left and its y
    /// to the back, so the two rows of the transform change their sign.
    fn volume(&self, of: &str, file: &str) -> Volume {
        let number = |row: usize, place: usize| self.case[of]["affine"][row][place].as_f64();
        let number = |row, place| number(row, place).expect("case.json places the volume");
        let column = |place| [-number(0, place), -number(1, place), number(2, place)];
        let placed = Affine::from_columns(column(0), column(1), column(2), column(3));
        Volume::new(self.sides(of), self.floats(file), placed).expect("a volume")
    }

    /// An image of the check folder, as a network is given one.
    fn image(&self, of: &str, file: &str) -> Tensor {
        let [x, y, z] = self.sides(of);
        Tensor::f32(vec![1, x, y, z, 1], self.floats(file)).expect("an image of those sides")
    }
}

/// Does a piece of labelling with the real networks on one device, and prints what it took.
fn on<T>(
    (device, name): (Device, &str),
    what: &str,
    bound: &Bound,
    work: impl FnOnce(&mut OnDevice, &mut dyn FnMut(Stage)) -> T,
) -> T {
    let mut networks = OnDevice::new(&REAL.0, &REAL.1, device);
    let (answer, took) = measured(&mut networks, work);
    println!("{what} on {name}: {}", took.words);
    // The card that was asked for is the card that worked.
    if let Some(rose) = took.card_rose {
        assert!(rose >= CARD_ROSE, "{name} held only {rose} bytes more");
    }
    // And the process asked the system for no more than the run is allowed.
    let held = bound.hold(device, took.asked);
    println!("  {}", held.unwrap_or_else(|over| panic!("{over}")));
    answer
}

/// Runs a model on Python's own image of the check volume, tidies what the networks find
/// onto the volume's grid, and holds the labels and the scores to Python's by the marks.
fn held(device: (Device, &str), wanted: Wanted) {
    let model = if wanted.robust { "robust" } else { "standard" };
    let lists = &REAL.1.lists;
    let volume = CHECK.volume("volume", "volume.f32");
    let prepared = prepare(&volume, Cut::Whole).expect("the volume can be prepared");
    let image = CHECK.image("image", "image.f32");
    // Beside the run the test holds the volume, Python's image and the image it prepared.
    let voxels: usize = image.shape.iter().product();
    let arrays = [
        ("the volume", volume.values().len()),
        ("Python's image", voxels),
        (
            "the prepared image",
            prepared.image.shape.iter().product::<usize>(),
        ),
    ];
    let bound = Bound::new(voxels, wanted, &arrays);
    let what = format!("the {model} model");
    let (grid, scores) = on(device, &what, &bound, |networks, said| {
        let found = find(&image, lists, wanted, networks, said).expect("the networks run");
        said(Stage::Tidying);
        let grid = tidy(&found, lists, wanted, &prepared.back).expect("the answer is tidied");
        (grid, found.scores)
    });
    let labels = CHECK.labels(&format!("{model}.u16"));
    let voxels = volume.values().len();
    assert_eq!(
        (labels.len(), grid.ids.len()),
        (voxels, voxels),
        "the grids"
    );
    let agreed = Agreement::of(&labels, &grid.ids);
    let theirs = CHECK.scores(model);
    assert_eq!(scores.len(), theirs.len(), "how many scores there are");
    let finite = scores.iter().all(|score| score.is_finite());
    assert!(finite, "a score is not a number: {scores:?}");
    let apart = scores.iter().zip(&theirs);
    let apart = apart.map(|(&ours, theirs)| (f64::from(ours) - theirs).abs());
    let apart = apart.fold(0.0, f64::max);
    println!("  against Python: {agreed}; scores within {apart:.5}");
    if device.0 == Device::Processor && !wanted.robust {
        assert_eq!(agreed.differ, 0, "voxels that are not as Python has them");
    }
    let (same, least) = (agreed.same_percent(), agreed.least_dice());
    assert!(same >= SAME_PERCENT, "only {same}% of voxels agree");
    assert!(least >= DICE, "a Dice of {least}: {:?}", agreed.dice);
    assert!(apart <= SCORES, "the scores are {apart} from Python's");
}

/// [`held`] on every card of one kind, one after another.
fn held_on_each(kind: CardKind, wanted: Wanted) {
    let _turn = turn();
    // The test was asked for, so a missing folder is said even where there is no such card.
    let _both = (folder(RUNTIME), folder(MODELS));
    let found = cards_of(kind);
    if found.is_empty() {
        println!("skipped: this computer has no graphics card that is {kind:?}");
    }
    for card in found {
        let spare = budget(card.id).map(|bytes| bytes as f64 / 1_073_741_824.0);
        println!("{}: the system gives it {spare:.2?} GiB", card.name);
        held((Device::Card(card.id), &card.name), wanted);
    }
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 4 GiB of memory"]
fn the_standard_model_on_the_processor_gives_pythons_labels_at_every_voxel() {
    let _turn = turn();
    held((Device::Processor, "the processor"), STANDARD);
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 4 GiB of memory"]
fn the_robust_model_on_the_processor_gives_pythons_labels() {
    let _turn = turn();
    held((Device::Processor, "the processor"), ROBUST);
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 9 GiB on the card; skips without a discrete card"]
fn the_standard_model_on_a_discrete_card_gives_pythons_labels() {
    held_on_each(CardKind::Discrete, STANDARD);
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 9 GiB on the card; skips without a discrete card"]
fn the_robust_model_on_a_discrete_card_gives_pythons_labels() {
    held_on_each(CardKind::Discrete, ROBUST);
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 9 GiB of memory; skips without integrated graphics"]
fn the_standard_model_on_integrated_graphics_gives_pythons_labels() {
    held_on_each(CardKind::Integrated, STANDARD);
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 9 GiB of memory; skips without integrated graphics"]
fn the_robust_model_on_integrated_graphics_gives_pythons_labels() {
    held_on_each(CardKind::Integrated, ROBUST);
}

/// Prepares a volume of the check folder and holds the image against Python's.
fn prepared_as_python_prepares(of: &str, file: &str, theirs: (&str, &str), within: f32) {
    let (volume, theirs) = (CHECK.volume(of, file), CHECK.image(theirs.0, theirs.1));
    let started = Instant::now();
    let ours = prepare(&volume, Cut::Whole).expect("the volume can be prepared");
    let seconds = started.elapsed().as_secs_f64();
    assert_eq!(ours.image.shape, theirs.shape, "the image's size");
    let (Numbers::F32(ours), Numbers::F32(theirs)) = (&ours.image.numbers, &theirs.numbers) else {
        panic!("an image is floating point numbers")
    };
    assert!(ours.iter().all(|value| value.is_finite()), "not a number");
    let apart = |(ours, theirs): (&f32, &f32)| (ours - theirs).abs();
    let furthest = ours.iter().zip(theirs).map(apart).fold(0.0, f32::max);
    let dims = volume.dims();
    println!("prepare of {of} {dims:?}: {seconds:.2} s, furthest from Python {furthest:e}");
    assert!(furthest <= within, "{furthest} is over the mark, {within}");
}

#[test]
#[ignore = "needs VOXA_MODELS"]
fn the_volume_is_prepared_as_python_prepares_it() {
    prepared_as_python_prepares("volume", "volume.f32", ("image", "image.f32"), IMAGE);
}

#[test]
#[ignore = "needs VOXA_MODELS"]
fn the_thick_copy_is_prepared_as_python_prepares_it() {
    let theirs = ("thick_image", "thick_image.f32");
    prepared_as_python_prepares("thick", "thick.f32", theirs, RESAMPLED_IMAGE);
}

/// How far along the patient's left-right axis the middle of a label's voxels lies, in
/// millimetres. Voxa's patient space grows towards the patient's left. `None` for a label
/// that too few voxels have.
fn leftness(grid: &LabelGrid, label: u32) -> Option<f64> {
    let [cols, rows, _] = grid.dims.map(|side| side as usize);
    let (mut sum, mut count) = ([0.0; 3], 0_u32);
    for (place, _) in grid.ids.iter().enumerate().filter(|(_, id)| **id == label) {
        let at = [place % cols, place / cols % rows, place / (cols * rows)];
        sum = [0, 1, 2].map(|axis| sum[axis] + at[axis] as f64);
        count += 1;
    }
    let [col, row, slice] = sum.map(|sum| sum / f64::from(count));
    (count >= ENOUGH).then(|| grid.placement.apply(Point::new(col, row, slice)).x)
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 9 GiB on a discrete card, or 4 GiB of memory"]
fn the_whole_path_puts_a_left_structure_on_the_patients_left_of_its_twin() {
    let _turn = turn();
    let lists = &REAL.1.lists;
    let (device, name) = quickest();
    let volume = CHECK.volume("volume", "volume.f32");
    // Beside the run the test holds the volume.
    let voxels: usize = CHECK.sides::<usize>("image").iter().product();
    let bound = Bound::new(voxels, ROBUST, &[("the volume", volume.values().len())]);
    let ours = on(
        (device, &name),
        "the whole path",
        &bound,
        |networks, said| {
            let labelled = label(&volume, Cut::Whole, lists, ROBUST, networks, said);
            labelled.expect("the scan is labelled")
        },
    );
    let theirs = LabelGrid {
        dims: volume.dims(),
        ids: CHECK.labels("robust.u16"),
        placement: volume.voxel_to_patient(),
    };
    // The whole path in one call gives Python's labels as the steps taken one by one do, says
    // nothing of a scan that is neither rough nor cut, and passes the model's own check.
    assert_eq!(ours.grid.dims, theirs.dims, "the grid's sides");
    assert_eq!(ours.grid.ids.len(), theirs.ids.len(), "the grid's voxels");
    let agreed = Agreement::of(&theirs.ids, &ours.grid.ids);
    println!("  against Python: {agreed}");
    let held = agreed.same_percent() >= SAME_PERCENT && agreed.least_dice() >= DICE;
    assert!(held, "the whole path misses the marks: {:?}", agreed.dice);
    let verdict = ours.table.verdict.as_ref();
    assert!(verdict.is_some_and(|verdict| verdict.passed), "{verdict:?}");
    assert!(ours.notes.is_empty(), "{:?}", ours.notes);
    let twins = lists.structures.iter().zip(&lists.flip);
    let mut pairs = 0;
    for (left, &twin) in twins.filter(|(part, _)| part.side == ModelSide::Left) {
        let right = lists.structures[twin].label;
        // A pair that Python found, each large enough to have a middle.
        let python = leftness(&theirs, left.label).zip(leftness(&theirs, right));
        let Some((their_left, their_right)) = python else {
            continue;
        };
        let voxa = leftness(&ours.grid, left.label).zip(leftness(&ours.grid, right));
        let (our_left, our_right) = voxa.expect("a pair that Python found and Voxa did not");
        let (apart, name) = (our_left - our_right, &left.name);
        println!("  the left {name} lies {apart:.1} mm to the left of the right one");
        let (python, voxa) = (their_left > their_right, our_left > our_right);
        assert!(python, "Python's left {name} lies on the right");
        assert!(voxa, "Voxa's left {name} lies on the right");
        pairs += 1;
    }
    assert!(pairs > 0, "no pair of twins to tell left from right by");
}
