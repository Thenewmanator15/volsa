//! Tests of tidying. The golden cases under `tests/golden/tidy` are what `SynthSeg`'s own
//! tidying made of made-up chances, with `SynthSeg`'s own lists; `tools/golden` makes them and
//! says what order each array is in. The other grids here are a line of voxels, and every
//! number in them is made up. What tidying refuses is in the file beside this one.

use std::path::Path;

use serde_json::Value;
use voxa_core::{Affine, Patient, Voxel};
use voxa_models::{Lists, ModelSide, Structure, Tensor, Wanted};

use super::{summed, tidy};
use crate::{Back, Found};

/// The standard network ran, or the robust ones. Tidying reads nothing else of what was wanted.
pub(super) const STANDARD: Wanted = Wanted {
    robust: false,
    cortex: false,
};
pub(super) const ROBUST: Wanted = Wanted {
    robust: true,
    cortex: false,
};
/// Where the golden cases are.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/tidy");

/// What one of the JSON files there says: `lists.json`, or a case's `case.json`.
fn json(file: &str) -> Value {
    let text = std::fs::read_to_string(Path::new(GOLDEN).join(file)).expect("it is there");
    serde_json::from_str(&text).expect("it is JSON")
}

/// The bytes of one of the arrays of the case called `name`.
fn array(name: &str, part: &Value) -> Vec<u8> {
    let file = part["file"].as_str().expect("a file name");
    std::fs::read(Path::new(GOLDEN).join(name).join(file)).expect("the array is there")
}

/// One of the arrays of the case called `name`, four bytes at a time: a number each.
fn fours(name: &str, part: &Value) -> Vec<[u8; 4]> {
    let bytes = array(name, part);
    let (fours, rest) = bytes.as_chunks::<4>();
    assert!(rest.is_empty(), "{name}: whole numbers");
    fours.to_vec()
}

fn triple(value: &Value) -> [usize; 3] {
    [0, 1, 2].map(|i| usize::try_from(value[i].as_u64().expect("a count")).expect("small"))
}

fn whole_numbers(value: &Value) -> Vec<u32> {
    let numbers = value.as_array().expect("a list").iter();
    let small = |number: &Value| u32::try_from(number.as_u64().expect("a whole number"));
    numbers
        .map(|number| small(number).expect("small"))
        .collect()
}

/// Lists for structures of these labels and classes, in this order, and for areas of these
/// labels. Tidying reads no more of the lists than that and the two cortex labels.
pub(super) fn lists(structures: &[(u32, u32)], areas: &[u32], cortex: [u32; 2]) -> Lists {
    let part = |label: u32, class: u32| Structure {
        label,
        name: format!("part {label}"),
        side: ModelSide::Midline,
        class,
    };
    Lists {
        structures: structures.iter().map(|&(l, c)| part(l, c)).collect(),
        flip: (0..structures.len()).collect(),
        stage_classes: 2,
        areas: areas.iter().map(|&label| part(label, 0)).collect(),
        cortex,
        score_class: vec![0; structures.len()],
        score_groups: Vec::new(),
        box_side: 32,
    }
}

/// `SynthSeg`'s own lists, as the generator wrote them down.
fn synthsegs_lists() -> Lists {
    let written = json("lists.json");
    let labels = whole_numbers(&written["labels"]);
    let classes = whole_numbers(&written["classes"]);
    let structures: Vec<(u32, u32)> = labels.into_iter().zip(classes).collect();
    let cortex = whole_numbers(&written["cortex"]);
    let areas = whole_numbers(&written["area_labels"]);
    lists(&structures, &areas, [cortex[0], cortex[1]])
}

/// A made-up place in the patient. Tidying hands it on and never reads it.
fn placed() -> Affine<Voxel, Patient> {
    let columns = [0.0, 1.0, 0.0];
    Affine::from_columns(
        columns,
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        [6.0, -8.0, 9.25],
    )
}

/// What the networks found, of `count` structures on a padded grid of this size.
pub(super) fn found(
    padded: [usize; 3],
    count: usize,
    chances: Vec<f32>,
    areas: Option<Vec<u8>>,
) -> Found {
    let [x, y, z] = padded;
    let areas = areas.map(|bytes| Tensor::u8(vec![1, x, y, z], bytes).expect("a byte a voxel"));
    Found {
        probabilities: Tensor::f32(vec![1, x, y, z, count], chances).expect("whole"),
        areas,
        scores: Vec::new(),
    }
}

/// A golden case: what Voxa has when it comes to tidy, and `SynthSeg`'s labels for the same.
pub(super) struct Case {
    found: Found,
    /// The robust networks, when `SynthSeg` tidied the case the way it calls fast.
    wanted: Wanted,
    pub(super) back: Back,
    labels: Vec<u32>,
}

fn case(name: &str) -> Case {
    let case = json(&format!("{name}/case.json"));
    let chances = fours(name, &case["chances"]).into_iter();
    let chances = chances.map(f32::from_le_bytes).collect();
    let areas = match &case["areas"] {
        Value::Null => None,
        areas => Some(array(name, areas)),
    };
    let count = usize::try_from(case["chances"]["structures"].as_u64().expect("a count"));
    let padded = triple(&case["chances"]["padded"]);
    let labels = fours(name, &case["labels"]).into_iter();
    let labels = labels.map(u32::from_le_bytes).collect();
    let back = &case["back"];
    let dims = triple(&back["grid_dims"]).map(|side| u32::try_from(side).expect("small"));
    assert_eq!(triple(&case["labels"]["dims"]), triple(&back["grid_dims"]));
    Case {
        found: found(padded, count.expect("small"), chances, areas),
        wanted: if case["fast"].as_bool().expect("yes or no") {
            ROBUST
        } else {
            STANDARD
        },
        back: Back {
            order: triple(&back["order"]),
            turned_round: [0, 1, 2].map(|i| back["turned_round"][i].as_bool().expect("yes or no")),
            padding_before: triple(&back["padding_before"]),
            size: triple(&back["size"]),
            cut_from: triple(&back["cut_from"]),
            grid_dims: dims,
            grid_placement: placed(),
        },
        labels,
    }
}

/// Tidies a golden case and holds every label against `SynthSeg`'s. Gives the case, for a test
/// to say what it shows.
pub(super) fn tidied_as_synthseg_tidies(name: &str, wanted: Wanted) -> Case {
    let case = case(name);
    assert_eq!(case.wanted, wanted, "{name}: the way SynthSeg tidied it");
    let lists = synthsegs_lists();
    let grid = tidy(&case.found, &lists, wanted, &case.back).expect("it is tidied");
    assert_eq!(grid.dims, case.back.grid_dims, "{name}");
    assert_eq!(grid.placement, placed(), "{name}");
    assert_eq!(grid.ids.len(), case.labels.len(), "{name}");
    let pairs = grid.ids.iter().zip(&case.labels).enumerate();
    let differ: Vec<_> = pairs.filter(|(_, (ours, theirs))| ours != theirs).collect();
    let (count, first) = (differ.len(), differ.first());
    assert_eq!(
        count, 0,
        "{name}: {count} labels differ; the first (place, (ours, SynthSeg's)) is {first:?}"
    );
    case
}

/// How many voxels of a case `SynthSeg` gave this label.
pub(super) fn count_of(case: &Case, label: u32) -> usize {
    case.labels.iter().filter(|&&held| held == label).count()
}

#[test]
fn of_two_pieces_of_one_class_the_smaller_goes_as_synthseg_has_it() {
    let case = tidied_as_synthseg_tidies("pieces", STANDARD);
    // What the case is there to show, read off SynthSeg's own labels: the left thalamus (10)
    // keeps its 12 voxels and loses its 4, and one putamen (12) of two as large is left.
    assert_eq!((count_of(&case, 10), count_of(&case, 12)), (12, 3));
}

#[test]
fn a_speck_far_off_goes_and_what_is_joined_through_the_padding_stays() {
    let case = tidied_as_synthseg_tidies("speck", STANDARD);
    // The brain stem (16) and the right ventral DC (60) are gone, the left one (28) is there.
    let kept = [16, 60, 28].map(|label| count_of(&case, label));
    assert_eq!(kept, [0, 0, 4]);
}

#[test]
fn the_cortex_takes_its_areas_and_a_cut_is_put_back_in_its_place() {
    let case = tidied_as_synthseg_tidies("cortex", STANDARD);
    // No voxel is left with a cortex label (3, 42), and the areas are there (1001 to 2035).
    assert_eq!((count_of(&case, 3), count_of(&case, 42)), (0, 0));
    let areas = case.labels.iter().filter(|&&label| label > 1000).count();
    assert_eq!(areas, 70, "35 voxels of cortex on each side");
    assert_ne!(case.back.cut_from, [0; 3], "the case has a cut");
}

#[test]
fn chances_on_the_edge_of_the_arithmetic_fall_as_synthsegs_do() {
    let case = tidied_as_synthseg_tidies("close", STANDARD);
    // The right thalamus (49) stays and the right caudate (50) goes; one voxel is right cortex
    // (42) and one, whose chances add to nought, left white matter (2).
    let kept = [49, 50, 42, 2].map(|label| count_of(&case, label));
    assert_eq!(kept, [2, 0, 1, 1]);
}

/// A line of voxels along x, stored as it is: no padding, no cut, nothing turned.
pub(super) fn line(count: usize) -> Back {
    Back {
        order: [0, 1, 2],
        turned_round: [false; 3],
        padding_before: [0; 3],
        size: [count, 1, 1],
        cut_from: [0; 3],
        grid_dims: [u32::try_from(count).expect("small"), 1, 1],
        grid_placement: placed(),
    }
}

/// What tidying makes of a line of voxels with these chances of three structures each, when
/// these networks made them.
pub(super) fn tidied_line(
    lists: &Lists,
    wanted: Wanted,
    voxels: &[[f32; 3]],
    areas: Option<Vec<u8>>,
) -> Vec<u32> {
    let found = found([voxels.len(), 1, 1], 3, voxels.concat(), areas);
    let back = line(voxels.len());
    tidy(&found, lists, wanted, &back)
        .expect("it is tidied")
        .ids
}

/// Eleven voxels: nothing with structure A close behind it; then A, B, A with B behind it, B
/// and A; four of nothing at all; and B again.
const ELEVEN: [[f32; 3]; 11] = [
    [0.5, 0.375, 0.125],
    [0.25, 0.75, 0.0],
    [0.25, 0.0, 0.75],
    [0.2, 0.5, 0.3],
    [0.25, 0.0, 0.75],
    [0.25, 0.75, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.25, 0.0, 0.75],
];

#[test]
fn a_class_is_whatever_number_the_lists_give_and_the_lowest_is_left_alone() {
    // The last voxel is not joined to the rest, so it is nothing whatever the classes. A is
    // labelled 7 and B 9. Each answer here is the one SynthSeg's own tidying gives.
    let (a, b) = (7, 9);
    // One class for all: only the piece of everything is kept.
    let one = lists(&[(0, 1), (a, 1), (b, 1)], &[], [3, 42]);
    let tidied = tidied_line(&one, STANDARD, &ELEVEN, None);
    assert_eq!(tidied, [0, a, b, a, b, a, 0, 0, 0, 0, 0]);
    // A has a class of its own, the highest there can be. Its first piece is the largest, of
    // two voxels: the others go, and B, which was behind one of them, shows. B is of the
    // lowest class, with nothing: its pieces all stay.
    let high = lists(&[(0, 4), (a, u32::MAX), (b, 4)], &[], [3, 42]);
    let tidied = tidied_line(&high, STANDARD, &ELEVEN, None);
    assert_eq!(tidied, [0, a, b, b, b, 0, 0, 0, 0, 0, 0]);
    // A is of the lowest class, so all its pieces stay. Nothing and B are one class above
    // it, and SynthSeg treats nothing there as it treats any structure. A voxel of nothing
    // counts wherever it lies, so the class's largest piece is the four voxels of nothing
    // at all. Outside it both nothing and B are put at nought: where B was, no chance is
    // left, and in the first voxel only A's.
    let low = lists(&[(0, 9), (a, 2), (b, 9)], &[], [3, 42]);
    let tidied = tidied_line(&low, STANDARD, &ELEVEN, None);
    assert_eq!(tidied, [a, a, 0, a, 0, a, 0, 0, 0, 0, 0]);
}

#[test]
fn cortex_takes_the_label_of_the_area_at_its_voxel_and_nothing_else_does() {
    // Left cortex (3), right cortex (42) and nothing; the areas are nothing, 1001 and 2001.
    let lists = lists(&[(0, 0), (3, 0), (42, 0)], &[0, 1001, 2001], [3, 42]);
    let left = [0.25, 0.75, 0.0];
    let right = [0.25, 0.0, 0.75];
    let nothing = [0.625, 0.25, 0.125];
    let voxels = [left, right, left, right, nothing];
    // The third voxel's area is the first of the list, which is nothing. The fourth names an
    // area the list does not have, so it is left as cortex.
    let areas = vec![1, 2, 0, 9, 2];
    let with_areas = tidied_line(&lists, STANDARD, &voxels, Some(areas));
    assert_eq!(with_areas, [1001, 2001, 0, 42, 0]);
    let without = tidied_line(&lists, STANDARD, &voxels, None);
    assert_eq!(without, [3, 42, 3, 42, 0]);
}

#[test]
fn a_voxels_chances_are_added_in_the_order_numpy_adds_them() {
    // One, and eight numbers too small to change it when added to it one at a time. Added in
    // turn all eight are lost; eight abreast, four of them come through.
    let mut numbers = [2.0_f32.powi(-25); 9];
    numbers[0] = 1.0;
    assert_eq!(numbers.iter().sum::<f32>(), 1.0);
    assert_eq!(summed(&numbers), 1.0 + f32::EPSILON);
    assert_eq!(summed(&numbers[..7]), 1.0, "under eight are added in turn");
    assert_eq!(summed(&[]), 0.0);

    // Lists of each length at which NumPy's way changes, with NumPy's own sums.
    let written = json("sums.json");
    let lists = written["sums"].as_array().expect("a list");
    #[expect(
        clippy::cast_possible_truncation,
        reason = "written from 32-bit floats"
    )]
    let single = |number: &Value| number.as_f64().expect("a number") as f32;
    let mut lengths = Vec::new();
    for list in lists {
        let numbers = list["numbers"].as_array().expect("a list").iter();
        let numbers: Vec<f32> = numbers.map(single).collect();
        let (ours, theirs) = (summed(&numbers), single(&list["sum"]));
        assert_eq!(
            ours.to_bits(),
            theirs.to_bits(),
            "{} numbers",
            numbers.len()
        );
        lengths.push(numbers.len());
    }
    assert_eq!(lengths, [7, 9, 15, 33, 130, 255]);
}
