//! Tests of preparing a volume. The golden cases under `tests/golden/prepare` are what
//! `SynthSeg`'s own Python made of small made-up volumes; `tools/golden` makes them and says
//! what order each array is in.

use std::path::Path;

use serde_json::Value;
use volsa_core::{Affine, Patient, Point, Volume, Voxel};
use volsa_models::{Cut, Numbers};

use super::PrepareError::{self, NothingToSee, TooFewSlices, TooLarge};
use super::align::Turn;
use super::resample::reflected;
use super::{Prepared, prepare, resampled};

type Placement = Affine<Voxel, Patient>;

/// Columns to the patient's right, rows to the front and slices to the top, as `SynthSeg`
/// wants them, these many millimetres apart.
fn stepped([across, down, through]: [f64; 3]) -> Placement {
    let columns = [-across, 0.0, 0.0];
    let rows = [0.0, -down, 0.0];
    Affine::from_columns(columns, rows, [0.0, 0.0, through], [0.0; 3])
}

/// A volume whose value at each voxel is `value(column, row, slice)`.
fn volume_of<F: Fn(u32, u32, u32) -> f32>(dims: [u32; 3], at: Placement, value: F) -> Volume {
    let [cols, rows, slices] = dims;
    let mut values = Vec::new();
    for slice in 0..slices {
        for row in 0..rows {
            values.extend((0..cols).map(|col| value(col, row, slice)));
        }
    }
    Volume::new(dims, values, at).expect("a volume")
}

/// A value that rises along every axis: the first voxel is the darkest.
fn ramp(col: u32, row: u32, slice: u32) -> f32 {
    (10 + col + 8 * row + 64 * slice) as f32
}

/// A volume of rising values, its axes as `SynthSeg` wants them and so many millimetres apart.
fn block(dims: [u32; 3], steps: [f64; 3]) -> Volume {
    volume_of(dims, stepped(steps), ramp)
}

fn whole(volume: &Volume) -> Prepared {
    prepare(volume, Cut::Whole).expect("it prepares")
}

fn refused(volume: &Volume) -> PrepareError {
    prepare(volume, Cut::Whole).expect_err("it is refused")
}

/// The image's numbers: `[1, x, y, z, 1]`, z fastest.
fn numbers(prepared: &Prepared) -> &[f32] {
    match &prepared.image.numbers {
        Numbers::F32(numbers) => numbers,
        _ => panic!("an image is floating point"),
    }
}

/// The image's value at `x`, `y`, `z`, counted from where the padding ends.
fn at(prepared: &Prepared, x: usize, y: usize, z: usize) -> f32 {
    let [before_x, before_y, before_z] = prepared.back.padding_before;
    let shape = &prepared.image.shape;
    numbers(prepared)[((x + before_x) * shape[2] + y + before_y) * shape[3] + z + before_z]
}

#[expect(clippy::cast_possible_truncation, reason = "a 32-bit float, or small")]
fn narrowed(value: f64) -> f32 {
    value as f32
}

/// Where the golden cases are.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/prepare");

/// What one of the JSON files there says: `turns.json`, or a case's `case.json`.
fn json(file: &str) -> Value {
    let text = std::fs::read_to_string(Path::new(GOLDEN).join(file)).expect("it is there");
    serde_json::from_str(&text).expect("it is JSON")
}

/// One of the arrays of the case called `name`: little-endian 32-bit floats.
fn floats(name: &str, case: &Value, part: &str) -> Vec<f32> {
    let file = case[part]["file"].as_str().expect("a file name");
    let file = Path::new(GOLDEN).join(name).join(file);
    let bytes = std::fs::read(file).expect("the array is there");
    let (fours, _) = bytes.as_chunks::<4>();
    fours.iter().map(|&four| f32::from_le_bytes(four)).collect()
}

/// A case's made-up volume, placed exactly as `SynthSeg` read it.
fn volume(name: &str, case: &Value) -> Volume {
    let dims = triple(&case["volume"]["dims"]).map(|side| u32::try_from(side).expect("small"));
    let placement = placement(&case["volume"]["to_patient"], true);
    Volume::new(dims, floats(name, case, "volume"), placement).expect("a volume")
}

fn triple(value: &Value) -> [usize; 3] {
    [0, 1, 2].map(|i| usize::try_from(value[i].as_u64().expect("a count")).expect("small"))
}

fn flags(value: &Value) -> [bool; 3] {
    [0, 1, 2].map(|i| value[i].as_bool().expect("yes or no"))
}

/// A transform from a case. The transform of a made-up volume is held as 32-bit floats, as
/// the NIfTI file `SynthSeg` read held it; with `single`, each number is taken as one, which
/// gives `SynthSeg`'s transform to the last bit however the JSON was parsed.
fn placement(value: &Value, single: bool) -> Placement {
    let exact = |n: f64| if single { f64::from(narrowed(n)) } else { n };
    let three = |value: &Value| [0, 1, 2].map(|i| exact(value[i].as_f64().expect("a number")));
    let [x, y, z] = [0, 1, 2].map(|column| three(&value["columns"][column]));
    Affine::from_columns(x, y, z, three(&value["origin"]))
}

/// The largest difference between the numbers of two transforms.
fn apart(a: &Placement, b: &Placement) -> f64 {
    let numbers = |t: &Placement| [t.columns().concat(), t.origin().to_vec()].concat();
    let pairs = numbers(a).into_iter().zip(numbers(b));
    pairs.map(|(a, b)| (a - b).abs()).fold(0.0, f64::max)
}

/// Prepares a case's volume and holds the result against Python's. The sizes and everything
/// `Back` remembers must be the same, and the padding zeros. Gives the largest difference
/// between the two images.
fn image_against_python(name: &str) -> f64 {
    let case = json(&format!("{name}/case.json"));
    let volume = volume(name, &case);
    let prepared = whole(&volume);
    let image = &case["image"];
    let of = |key: &str| triple(&image[key]);
    let (size, padded, before) = (of("size"), of("padded"), of("padding_before"));
    let [wide, long, tall] = padded;
    assert_eq!(prepared.image.shape, [1, wide, long, tall, 1], "{name}");
    let back = &prepared.back;
    let kept = [back.size, back.padding_before, back.cut_from];
    assert_eq!(kept, [size, before, [0; 3]], "{name}");
    assert_eq!(back.order, of("order"), "{name}");
    assert_eq!(back.turned_round, flags(&image["turned_round"]), "{name}");
    let grid = match &case["resampled"] {
        Value::Null => &case["volume"],
        resampled => resampled,
    };
    let dims = back.grid_dims.map(|side| side as usize);
    assert_eq!(dims, triple(&grid["dims"]), "{name}");
    let placed = placement(&grid["to_patient"], false);
    let off = apart(&back.grid_placement, &placed);
    assert!(off < 1e-9, "{name}: the grid is placed {off} mm away");
    assert_eq!(prepared.spacing_mm, volume.spacing_mm(), "{name}");

    let theirs = floats(name, &case, "image");
    assert_eq!(theirs.len(), size.iter().product::<usize>(), "{name}");
    let mut worst = 0.0_f64;
    for (place, &ours) in numbers(&prepared).iter().enumerate() {
        let at = [place / (long * tall), place / tall % long, place % tall];
        let inside = |axis: usize| (before[axis]..before[axis] + size[axis]).contains(&at[axis]);
        if (0..3).all(inside) {
            let [x, y, z] = [0, 1, 2].map(|axis| at[axis] - before[axis]);
            let expected = theirs[(x * size[1] + y) * size[2] + z];
            worst = worst.max((f64::from(ours) - f64::from(expected)).abs());
        } else {
            assert_eq!(ours, 0.0, "{name}: the padding is zeros");
        }
    }
    println!("{name}: the image is at most {worst:e} from Python's");
    worst
}

#[test]
fn a_volume_at_one_millimetre_gives_pythons_image_to_within_rounding() {
    let worst = ["iso", "sagittal", "coronal", "tilted"].map(image_against_python);
    assert!(worst.iter().all(|&off| off <= 0.000_01), "{worst:?}");
}

#[test]
fn a_resampled_volume_gives_pythons_image_to_within_a_ten_thousandth() {
    let worst = ["thick", "fine", "uneven"].map(image_against_python);
    assert!(worst.iter().all(|&off| off <= 0.000_1), "{worst:?}");
}

/// `uneven` is the case where `SynthSeg`'s rule gives one sample more along an axis than the
/// spacing times the count, rounded up: the sizes here are `SynthSeg`'s, not that sum's.
#[test]
fn resampling_gives_pythons_one_millimetre_volume() {
    for name in ["thick", "fine", "uneven"] {
        let case = json(&format!("{name}/case.json"));
        let ours = resampled(&volume(name, &case));
        let theirs = &case["resampled"];
        let dims = ours.dims().map(|side| side as usize);
        assert_eq!(dims, triple(&theirs["dims"]), "{name}");
        let placed = placement(&theirs["to_patient"], false);
        let off = apart(&ours.voxel_to_patient(), &placed);
        assert!(off < 1e-9, "{name}: placed {off} mm away");
        let expected = floats(name, &case, "resampled");
        assert_eq!(ours.values().len(), expected.len(), "{name}");
        let lowest = expected.iter().copied().fold(f32::MAX, f32::min);
        let highest = expected.iter().copied().fold(f32::MIN, f32::max);
        let differ = |(&a, &b): (&f32, &f32)| (f64::from(a) - f64::from(b)).abs();
        let pairs = ours.values().iter().zip(&expected);
        // As a part of the span of the values, which is what the image is scaled by. The
        // mark is rounding, a hundredth of the image's: a blur along an axis that should have
        // none, or a line carried on wrongly past its ends, is smaller than the image's mark.
        let worst = pairs.map(differ).fold(0.0, f64::max) / f64::from(highest - lowest);
        println!("{name}: the 1 mm volume is at most {worst:e} of its span from Python's");
        assert!(worst <= 0.000_001, "{name}: {worst}");
    }
}

#[test]
fn the_axes_are_turned_as_synthseg_turns_them() {
    let turns = json("turns.json");
    let turns = turns["turns"].as_array().expect("a list");
    assert!(turns.len() >= 10, "{} turns", turns.len());
    for turn in turns {
        let placed = placement(&turn["to_patient"], true);
        let speck = Volume::new([1, 1, 1], vec![0.0], placed).expect("a volume");
        let found = Turn::of(&speck, [1.0; 3]);
        assert_eq!(found.order, triple(&turn["order"]), "{turn}");
        assert_eq!(found.turned_round, flags(&turn["turned_round"]), "{turn}");
    }
}

#[test]
fn sides_are_padded_with_zeros_to_a_multiple_of_32_and_at_least_128() {
    let prepared = whole(&block([100, 129, 160], [1.0; 3]));
    assert_eq!(prepared.image.shape, [1, 128, 160, 160, 1]);
    assert_eq!(prepared.back.size, [100, 129, 160]);
    assert_eq!(prepared.back.padding_before, [14, 15, 0]);
    let image = numbers(&prepared);
    assert_eq!(image.len(), 128 * 160 * 160);
    // The brightest corner of the volume, counted without the padding and with it.
    assert_eq!(at(&prepared, 99, 128, 159), 1.0);
    let padded = |x: usize, y: usize, z: usize| image[(x * 160 + y) * 160 + z];
    assert_eq!(padded(113, 143, 159), 1.0);
    assert_eq!(padded(114, 143, 159), 0.0, "after it along x");
    assert_eq!(padded(113, 144, 159), 0.0, "after it along y");
    assert_eq!(padded(13, 143, 159), 0.0, "before the volume along x");
    assert_eq!(padded(113, 14, 159), 0.0, "before the volume along y");
}

#[test]
fn brightness_is_spread_between_the_two_percentiles() {
    // The whole numbers 0 to 1000 in order: the 0.5th percentile is 5 and the 99.5th is 995.
    let counted = |col: u32, row: u32, slice: u32| ((slice * 11 + row) * 7 + col) as f32;
    let prepared = whole(&volume_of([7, 11, 13], stepped([1.0; 3]), counted));
    let of = |value: usize| at(&prepared, value % 7, value / 7 % 11, value / 77);
    assert_eq!(of(5), 0.0);
    assert_eq!(of(995), 1.0);
    assert_eq!(of(500), 0.5);
    assert_eq!((of(203), of(797)), (0.2, 0.8));
    assert_eq!((of(0), of(4)), (0.0, 0.0), "held at the low end");
    assert_eq!((of(996), of(1000)), (1.0, 1.0), "held at the high end");
}

#[test]
fn the_patients_left_is_at_low_x_however_the_volume_is_stored() {
    // Where the columns, rows and slices of an axial, a sagittal and a coronal series run in
    // the patient, whose axes are left, back and top; and how each must be turned.
    let [left, back, top] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let down = [0.0, 0.0, -1.0];
    let stored = [
        ([left, back, top], [0, 1, 2], [true, true, false]),
        ([back, down, left], [2, 0, 1], [true; 3]),
        ([left, down, back], [0, 2, 1], [true; 3]),
    ];
    for ([columns, rows, slices], order, turned_round) in stored {
        let placement = Affine::from_columns(columns, rows, slices, [-3.0, 4.0, 5.0]);
        // Brighter the further to the patient's left a voxel lies.
        let leftness = |col: u32, row: u32, slice: u32| {
            let voxel = Point::new(f64::from(col), f64::from(row), f64::from(slice));
            narrowed(placement.apply(voxel).x)
        };
        let prepared = whole(&volume_of([5, 11, 12], placement, leftness));
        let turn = (prepared.back.order, prepared.back.turned_round);
        assert_eq!(turn, (order, turned_round));
        let [across, deep, high] = prepared.back.size;
        for (y, z) in (0..deep).flat_map(|y| (0..high).map(move |z| (y, z))) {
            let line: Vec<f32> = (0..across).map(|x| at(&prepared, x, y, z)).collect();
            assert!(line.is_sorted_by(|a, b| a >= b), "{order:?}: {line:?}");
            assert!(line.first() > line.last(), "{order:?}: {line:?}");
        }
    }
}

#[test]
fn under_four_voxels_on_a_side_is_too_few_slices() {
    assert_eq!(refused(&block([8, 8, 3], [1.0; 3])), TooFewSlices);
    assert_eq!(refused(&block([3, 8, 11], [1.0; 3])), TooFewSlices);
    assert_eq!(whole(&block([4, 8, 11], [1.0; 3])).back.size, [4, 8, 11]);
    // Three slices 5 mm apart would be 15 at 1 mm, and are still too few.
    assert_eq!(refused(&block([8, 8, 3], [1.0, 1.0, 5.0])), TooFewSlices);
    // Twelve slices a fifth of a millimetre apart are under four at 1 mm.
    let slab = block([8, 8, 12], [1.0, 1.0, 0.2]);
    assert_eq!(refused(&slab), TooFewSlices);
    assert_eq!(resampled(&slab), slab, "and it is not resampled");
}

#[test]
fn one_value_throughout_is_nothing_to_see() {
    // One value; one voxel in 704 unlike the rest, too few to move either percentile; and
    // no numbers at all.
    let fills: [fn(u32, u32, u32) -> f32; 3] = [
        |_, _, _| 7.0,
        |col, row, slice| if col + row + slice == 0 { 100.0 } else { 7.0 },
        |_, _, _| f32::NAN,
    ];
    for fill in fills {
        let volume = volume_of([8, 8, 11], stepped([1.0; 3]), fill);
        assert_eq!(refused(&volume), NothingToSee);
    }
}

#[test]
fn a_grid_over_512_voxels_on_a_side_is_too_large() {
    assert_eq!(refused(&block([600, 8, 11], [1.0; 3])), TooLarge);
    let just = whole(&block([512, 4, 11], [1.0; 3]));
    assert_eq!(just.image.shape, [1, 512, 128, 128, 1]);
    // 300 columns 2 mm apart are 600 at 1 mm.
    let wide = block([300, 8, 11], [2.0, 1.0, 1.0]);
    assert_eq!(refused(&wide), TooLarge);
    assert_eq!(resampled(&wide), wide, "and it is not resampled");
    // A spacing past all counting is refused like any other, and stops nothing: the last of
    // these is too small for its grid to be counted at all.
    let vast = block([8, 8, 11], [1e100, 1.0, 1.0]);
    let minute = block([8, 8, 11], [1e-100, 1.0, 1.0]);
    assert_eq!((refused(&vast), refused(&minute)), (TooLarge, TooFewSlices));
    assert_eq!(resampled(&minute), minute);
    let uncounted = block([8, 8, 11], [1e-320, 1.0, 1.0]);
    assert_eq!(refused(&uncounted), TooFewSlices);
}

#[test]
fn a_volume_within_a_twentieth_of_a_millimetre_of_1_is_not_resampled() {
    let near = block([6, 5, 11], [1.02, 0.96, 1.04]);
    assert_eq!(resampled(&near), near);
    let prepared = whole(&near);
    assert_eq!(prepared.back.grid_dims, [6, 5, 11]);
    assert_eq!(prepared.back.grid_placement, near.voxel_to_patient());
    assert_eq!(prepared.spacing_mm, [1.02, 0.96, 1.04]);
    // A little further from 1 mm along one axis, and every axis is resampled.
    let far = block([6, 5, 11], [1.02, 0.96, 1.5]);
    let at_one_mm = resampled(&far);
    assert_eq!(at_one_mm.dims(), [7, 5, 17]);
    let steps = at_one_mm.spacing_mm();
    assert!(
        steps.iter().all(|step| (step - 1.0).abs() < 1e-12),
        "{steps:?}"
    );
    let prepared = whole(&far);
    assert_eq!(prepared.back.grid_dims, [7, 5, 17]);
    assert_eq!(prepared.back.grid_placement, at_one_mm.voxel_to_patient());
    assert_eq!(prepared.spacing_mm, [1.02, 0.96, 1.5], "as it was given");
}

#[test]
fn a_value_that_is_not_finite_counts_as_the_smallest() {
    // Without resampling and with it: the values are mended before anything else.
    for steps in [[1.0; 3], [0.5, 1.0, 1.0]] {
        let spoiled = |col, row, slice| match (col, row, slice) {
            (3, 3, 3) => f32::NAN,
            (4, 1, 6) => f32::INFINITY,
            (5, 2, 0) => f32::NEG_INFINITY,
            _ => ramp(col, row, slice),
        };
        let mended = |col, row, slice| match (col, row, slice) {
            (3, 3, 3) | (4, 1, 6) | (5, 2, 0) => 10.0,
            _ => ramp(col, row, slice),
        };
        let prepared = whole(&volume_of([8, 8, 11], stepped(steps), spoiled));
        let same = whole(&volume_of([8, 8, 11], stepped(steps), mended));
        assert_eq!(prepared, same);
        assert_eq!(numbers(&prepared).len(), 128 * 128 * 128);
        let [across, deep, high] = prepared.back.size;
        assert_eq!(at(&prepared, 0, 0, 0), 0.0, "the smallest value");
        let last = at(&prepared, across - 1, deep - 1, high - 1);
        assert_eq!(last, 1.0, "the largest value");
    }
}

#[test]
fn a_line_is_carried_on_past_its_ends_as_scipy_reflects_it() {
    // d c b a | a b c d | d c b a
    let carried: Vec<usize> = (-6..10_isize).map(|place| reflected(place, 4)).collect();
    assert_eq!(carried, [2, 3, 3, 2, 1, 0, 0, 1, 2, 3, 3, 2, 1, 0, 0, 1]);
    assert_eq!(reflected(-3, 1), 0);
}

#[test]
fn every_refusal_is_a_plain_sentence() {
    let refusals = [TooFewSlices, NothingToSee, TooLarge];
    for (refusal, word) in refusals.into_iter().zip(["four", "differ", "512"]) {
        let said = refusal.to_string();
        assert!(said.contains(word) && said.ends_with('.'), "{said}");
        assert!(!said.contains("Prepare") && !said.contains("::"), "{said}");
    }
}
