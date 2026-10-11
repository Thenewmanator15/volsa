//! Tests of the scoring: the box cut from the map of top structures, and what is made of the
//! scores network's answer. The cases under `tests/golden/box` are what `SynthSeg`'s own layer
//! made of small made-up maps, for a box of 8; `tools/golden` makes them and says what each
//! file holds.

use std::path::Path;

use serde_json::Value;
use voxa_models::{Lists, Role, Tensor};

use super::tests::{
    BOX, GRID, Shelf, image, lists, no_scores, plain_image, put_top, run, three, wanted,
};

/// Where the golden cases are.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/box");

/// One of Python's cases.
struct Case {
    /// The grid's size along x, y and z.
    size: [usize; 3],
    /// How many structures a voxel's place is one of.
    structures: u32,
    /// The place of the top structure at each voxel of the grid, z fastest.
    places: Vec<u8>,
    /// The side of the box.
    side: usize,
    /// The box `SynthSeg`'s layer made of the grid, z fastest.
    boxed: Vec<u8>,
}

fn case(name: &str) -> Case {
    let folder = Path::new(GOLDEN).join(name);
    let text = std::fs::read_to_string(folder.join("case.json")).expect("the case is there");
    let json: Value = serde_json::from_str(&text).expect("it is JSON");
    let count = |value: &Value| usize::try_from(value.as_u64().expect("a count")).expect("small");
    let bytes = |part: &str| {
        let file = json[part]["file"].as_str().expect("a file name");
        std::fs::read(folder.join(file)).expect("the array is there")
    };
    Case {
        size: [0, 1, 2].map(|axis| count(&json["places"]["size"][axis])),
        structures: u32::try_from(count(&json["places"]["structures"])).expect("small"),
        places: bytes("places"),
        side: count(&json["box"]["side"]),
        boxed: bytes("box"),
    }
}

/// What the scores network is given when the top structure of each voxel is the one `top`
/// gives. It must have been opened for the box, whatever the size of the grid.
fn given_to_scores(lists: &Lists, robust: bool, size: [usize; 3], top: &[u8]) -> Tensor {
    let mut shelf = Shelf::default();
    put_top(&mut shelf, robust, size, lists.structures.len(), top);
    let scores = shelf.put(Role::Scores, no_scores);
    let seen = image(size, vec![0.5; size.iter().product()]);
    let (found, _) = run(&seen, lists, wanted(robust, false), &mut shelf);
    found.expect("the image is labelled");
    let opened_for = [lists.box_side; 3];
    assert_eq!(shelf.opened.last(), Some(&(Role::Scores, opened_for)));
    let mut runs = scores.runs();
    assert_eq!(runs.len(), 1, "the scores network runs once");
    let mut inputs = runs.remove(0);
    assert_eq!(inputs.len(), 1, "and is given one thing");
    inputs.remove(0)
}

/// Holds the box Voxa cuts from a case's map against the one Python cut.
fn the_box_is_pythons(name: &str) {
    let case = case(name);
    // Each place is a score class of its own here, so the box holds places, as Python's does.
    let places: Vec<u32> = (0..case.structures).collect();
    let stay: Vec<usize> = (0..places.len()).collect();
    let mut lists = lists(&places, &stay);
    lists.box_side = case.side;
    let python = Tensor::u8(vec![1, case.side, case.side, case.side], case.boxed);
    let python = python.expect("a box of bytes");
    for robust in [false, true] {
        let ours = given_to_scores(&lists, robust, case.size, &case.places);
        assert_eq!(ours, python, "{name}, robust {robust}");
    }
}

#[test]
fn a_blob_in_the_middle_is_boxed_as_python_boxes_it() {
    the_box_is_pythons("middle");
}

#[test]
fn a_blob_in_a_corner_is_boxed_as_python_boxes_it() {
    the_box_is_pythons("corner");
}

#[test]
fn a_blob_longer_than_the_box_is_boxed_as_python_boxes_it() {
    the_box_is_pythons("longer");
}

#[test]
fn a_map_with_no_blob_is_boxed_as_python_boxes_it() {
    the_box_is_pythons("none");
}

#[test]
fn each_place_in_the_box_becomes_its_score_class_and_so_does_the_nothing_round_the_grid() {
    let mut lists = three();
    lists.score_class = vec![2, 0, 1];
    // Place 2 at the first voxel, place 1 at every voxel of the two middle slabs along x,
    // and nothing elsewhere.
    let top = [2, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0];
    let given = given_to_scores(&lists, false, GRID, &top);

    // What counts runs from 0 to 3 along x, which leaves 5 of the box's 8 to share: 3 before
    // and 2 after. Along y and z it is 2 long, with 3 before and 3 after. So the grid's first
    // voxel lies at 3, 3, 3 of the box. Nothing is class 2, in the grid and round it.
    let at = |x: usize, y: usize, z: usize| (x * BOX + y) * BOX + z;
    let mut expected = vec![2_u8; BOX * BOX * BOX];
    expected[at(3, 3, 3)] = 1;
    for (x, y, z) in (0..8).map(|place| (4 + place / 4, 3 + place / 2 % 2, 3 + place % 2)) {
        expected[at(x, y, z)] = 0;
    }
    assert_eq!(
        given,
        Tensor::u8(vec![1, BOX, BOX, BOX], expected).expect("whole")
    );
}

/// The scores `find` gives when the scores network answers with these numbers.
fn scores_of(answer: [f32; 4]) -> Vec<f32> {
    let mut shelf = Shelf::default();
    put_top(&mut shelf, false, GRID, 3, &[1; 16]);
    let answer = Tensor::f32(vec![1, 4], answer.to_vec()).expect("whole");
    shelf.put(Role::Scores, move |_| Ok(vec![answer.clone()]));
    let (found, _) = run(&plain_image(), &three(), wanted(false, false), &mut shelf);
    found.expect("the image is labelled").scores
}

#[test]
fn scores_lose_the_first_number_and_are_held_from_nought_to_one_to_four_places() {
    assert_eq!(scores_of([9.0, 0.81234, 1.2, -0.1]), [0.8123, 1.0, 0.0]);
    // Rounded to the nearest and not cut short. The last is exactly half way between
    // 0.0312 and 0.0313, and a half goes away from nought.
    assert_eq!(
        scores_of([0.0, 0.56789, 0.99996, 0.03125]),
        [0.5679, 1.0, 0.0313]
    );
}

#[test]
fn the_place_that_is_left_out_is_told_by_its_place_and_not_by_its_label() {
    // Places 16 and 24 change labels: the structure at place 24 has another label, and
    // label 24 is at another place.
    let mut labels: Vec<u32> = (0..33).collect();
    labels.swap(16, 24);
    let stay: Vec<usize> = (0..33).collect();
    let lists = lists(&labels, &stay);
    // Place 24 fills the first slab along x. The last voxel of the grid is place 16.
    let top = [24, 24, 24, 24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 16];
    let given = given_to_scores(&lists, false, GRID, &top);

    // Only the last voxel counts. It is one voxel long each way, which leaves 7 to share: 4
    // before and 3 after. So it lies at 4, 4, 4 of the box, and the grid's first voxel at
    // 1, 3, 3. Were place 24 counted and place 16 left out, the grid's first voxel would lie
    // at 4, 3, 3.
    let at = |x: usize, y: usize, z: usize| (x * BOX + y) * BOX + z;
    let mut expected = vec![0_u8; BOX * BOX * BOX];
    for (y, z) in [(3, 3), (3, 4), (4, 3), (4, 4)] {
        expected[at(1, y, z)] = 24;
    }
    expected[at(4, 4, 4)] = 16;
    assert_eq!(
        given,
        Tensor::u8(vec![1, BOX, BOX, BOX], expected).expect("whole")
    );
}
