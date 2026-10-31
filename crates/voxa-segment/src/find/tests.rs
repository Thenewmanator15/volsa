//! Tests of the steps between the networks. Stand-ins answer for the networks, so each test
//! says what a network gives, and looks at what `find` hands on and what it makes of the
//! answers. The grids are tiny and every number is made up. No network is read or run.
//!
//! The stand-ins and the made-up images and lists are here, with the tests of the cortex, of
//! the top structure and of one network at a time. The tests of the networks that find the
//! structures, of the scores and of what is refused are in the files beside this one.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use voxa_models::testkit::{Canned, Given};
use voxa_models::{Lists, ModelSide, Network, Numbers, Role, RunError, Structure, Tensor, Wanted};

use super::Stage::{Checking, Cortex, Finding};
use super::{Found, Networks, Stage, find, top_structures};

/// What a stand-in answers.
pub(super) type Answer = Result<Vec<Tensor>, RunError>;

/// The grid of most tests here: 4 voxels along x, 2 along y and 2 along z.
pub(super) const GRID: [usize; 3] = [4, 2, 2];
/// The side of the box in the lists of these tests.
pub(super) const BOX: usize = 8;

/// The stand-in networks of a test. Each is handed out once. The shelf notes every opening,
/// and how many networks were still alive when another was asked for.
#[derive(Default)]
pub(super) struct Shelf {
    waiting: Vec<(Role, Canned)>,
    /// Every opening, in order: the network and the sizes it was opened for.
    pub(super) opened: Vec<(Role, [usize; 3])>,
    /// For every opening, how many networks handed out before it had not yet been dropped.
    pub(super) alive_at_opening: Vec<usize>,
    /// A network that cannot be opened, and why.
    pub(super) shut: Option<(Role, RunError)>,
    alive: Arc<AtomicUsize>,
}

impl Shelf {
    /// Puts a stand-in on the shelf. The handle says afterwards what it was given.
    pub(super) fn put(
        &mut self,
        role: Role,
        answer: impl FnMut(&[Tensor]) -> Answer + Send + 'static,
    ) -> Given {
        let canned = Canned::new(answer);
        let given = canned.given();
        self.waiting.push((role, canned));
        given
    }

    /// How many of the networks handed out have not been dropped.
    pub(super) fn alive(&self) -> usize {
        self.alive.load(Ordering::SeqCst)
    }
}

impl Networks for Shelf {
    fn open(&mut self, role: Role, sizes: [usize; 3]) -> Result<Box<dyn Network>, RunError> {
        self.opened.push((role, sizes));
        self.alive_at_opening.push(self.alive());
        if let Some((_, why)) = self.shut.as_ref().filter(|(shut, _)| *shut == role) {
            return Err(why.clone());
        }
        let place = self.waiting.iter().position(|(held, _)| *held == role);
        let place = place.unwrap_or_else(|| panic!("the test has no {role} network to open"));
        let (_, network) = self.waiting.remove(place);
        self.alive.fetch_add(1, Ordering::SeqCst);
        let alive = Arc::clone(&self.alive);
        Ok(Box::new(Watched { network, alive }))
    }
}

/// A stand-in that says when it is dropped.
struct Watched {
    network: Canned,
    alive: Arc<AtomicUsize>,
}

impl Network for Watched {
    fn run(&mut self, inputs: Vec<Tensor>) -> Answer {
        self.network.run(inputs)
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        self.alive.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(super) const fn wanted(robust: bool, cortex: bool) -> Wanted {
    Wanted { robust, cortex }
}

/// Lists for structures of these labels, in this order, mirrored as `flip` says. Each
/// structure is a score class of its own, there are three score groups, and the box is 8.
pub(super) fn lists(labels: &[u32], flip: &[usize]) -> Lists {
    let structure = |&label: &u32| Structure {
        label,
        name: format!("structure {label}"),
        side: ModelSide::Midline,
        class: 0,
    };
    Lists {
        structures: labels.iter().map(structure).collect(),
        flip: flip.to_vec(),
        stage_classes: 2,
        areas: Vec::new(),
        cortex: [3, 42],
        score_class: (0..labels.len()).collect(),
        score_groups: ["one", "two", "three"].map(str::to_owned).to_vec(),
        box_side: BOX,
    }
}

/// Three structures: nothing, and the cortex of the left and of the right, which change
/// places in a mirror.
pub(super) fn three() -> Lists {
    lists(&[0, 3, 42], &[0, 2, 1])
}

/// An image of this size along x, y and z, with z changing fastest.
pub(super) fn image(size: [usize; 3], values: Vec<f32>) -> Tensor {
    let [x, y, z] = size;
    Tensor::f32(vec![1, x, y, z, 1], values).expect("as many values as voxels")
}

/// An image of the grid of these tests that is the same everywhere.
pub(super) fn plain_image() -> Tensor {
    image(GRID, vec![0.5; 16])
}

pub(super) fn floats(tensor: &Tensor) -> &[f32] {
    match &tensor.numbers {
        Numbers::F32(numbers) => numbers,
        _ => panic!("floating point numbers were expected"),
    }
}

/// A map of the grid of these tests: a byte for each voxel.
pub(super) fn map(bytes: Vec<u8>) -> Tensor {
    Tensor::u8(vec![1, 4, 2, 2], bytes).expect("a byte for each voxel")
}

/// The chances of `count` structures at every voxel (the structures changing fastest) as a
/// network gives them: in two tensors, the first holding the first `wide` structures.
pub(super) fn halves(size: [usize; 3], count: usize, wide: usize, chances: &[f32]) -> Vec<Tensor> {
    let [x, y, z] = size;
    let voxels = || chances.chunks(count);
    let first: Vec<f32> = voxels().flat_map(|voxel| &voxel[..wide]).copied().collect();
    let second: Vec<f32> = voxels().flat_map(|voxel| &voxel[wide..]).copied().collect();
    vec![
        Tensor::f32(vec![1, x, y, z, wide], first).expect("the first half"),
        Tensor::f32(vec![1, x, y, z, count - wide], second).expect("the second half"),
    ]
}

/// The chances when a network is sure of every voxel: 1 for the voxel's structure in `top`
/// and 0 for every other.
pub(super) fn certain(top: &[u8], count: usize) -> Vec<f32> {
    let voxel =
        |&place: &u8| (0..count).map(move |channel| u8::from(channel == usize::from(place)));
    top.iter().flat_map(voxel).map(f32::from).collect()
}

/// Puts on the shelf the networks that find the structures, answering so that the top
/// structure of each voxel is the one `top` gives.
pub(super) fn put_top(shelf: &mut Shelf, robust: bool, size: [usize; 3], count: usize, top: &[u8]) {
    let chances = certain(top, count);
    let wide = count.div_ceil(2);
    if robust {
        let [x, y, z] = size;
        let nothing = move || Tensor::u8(vec![1, x, y, z], vec![0; x * y * z]).expect("a map");
        shelf.put(Role::Robust1, move |_| Ok(vec![nothing()]));
        shelf.put(Role::Robust2, move |_| Ok(vec![nothing()]));
        shelf.put(Role::Robust3, move |_| {
            Ok(halves(size, count, wide, &chances))
        });
    } else {
        // On the mirror the stand-in answers with nothing at all, so the mean is half the
        // first answer, and its largest is where the first answer's was.
        let mut on_the_image = true;
        shelf.put(Role::Standard, move |_| {
            let scale = if on_the_image { 1.0 } else { 0.0 };
            on_the_image = false;
            let scaled: Vec<f32> = chances.iter().map(|chance| chance * scale).collect();
            Ok(halves(size, count, wide, &scaled))
        });
    }
}

/// A stand-in for the scores network: the number that is dropped, and nought for each of
/// the three score groups.
#[expect(clippy::unnecessary_wraps, reason = "it answers as a network does")]
pub(super) fn no_scores(_: &[Tensor]) -> Answer {
    Ok(vec![Tensor::f32(vec![1, 4], vec![0.0; 4]).expect("whole")])
}

/// What `find` gives, and the stages it says on the way.
pub(super) fn run(
    image: &Tensor,
    lists: &Lists,
    wanted: Wanted,
    shelf: &mut Shelf,
) -> (Result<Found, RunError>, Vec<Stage>) {
    let mut stages = Vec::new();
    let found = find(image, lists, wanted, shelf, &mut |stage| stages.push(stage));
    (found, stages)
}

#[test]
fn the_cortex_network_is_given_a_one_wherever_the_top_structure_is_cortex() {
    // The cortex of the left and of the right, labels 3 and 42, are at places 2 and 4.
    let lists = lists(&[0, 2, 3, 41, 42], &[0, 3, 4, 1, 2]);
    let top = [0, 1, 2, 3, 4, 4, 3, 2, 1, 0, 2, 2, 4, 1, 3, 0];
    let cortex = [0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 1, 1, 1, 0, 0, 0];
    let areas = map((1..=16).collect());
    for robust in [false, true] {
        let mut shelf = Shelf::default();
        put_top(&mut shelf, robust, GRID, 5, &top);
        let answer = areas.clone();
        let given = shelf.put(Role::Cortex, move |_| Ok(vec![answer.clone()]));
        shelf.put(Role::Scores, no_scores);
        let seen = plain_image();
        let (found, stages) = run(&seen, &lists, wanted(robust, true), &mut shelf);
        let found = found.expect("the image is labelled");

        assert_eq!(
            given.runs(),
            vec![vec![seen, map(cortex.to_vec())]],
            "{robust}"
        );
        assert_eq!(found.areas, Some(areas.clone()), "{robust}");
        // After the structures are found and before the result is scored.
        let finding = if robust { 3 } else { 2 };
        assert_eq!(stages[finding..], [Cortex, Checking], "{robust}");
        assert!(
            stages[..finding]
                .iter()
                .all(|stage| matches!(stage, Finding { .. }))
        );
        let last_two = [(Role::Cortex, GRID), (Role::Scores, [BOX; 3])];
        assert!(shelf.opened.ends_with(&last_two), "{:?}", shelf.opened);
    }
}

#[test]
fn the_top_structure_is_the_largest_and_the_first_of_those_that_are_equal() {
    #[rustfmt::skip]
    let chances = [
        0.125, 0.5, 0.25, 0.0, 0.125,
        0.125, 0.25, 0.25, 0.25, 0.125,
        0.25, 0.125, 0.25, 0.125, 0.25,
        0.0, 0.125, 0.125, 0.25, 0.5,
        0.0, 0.0, 0.0, 0.0, 0.0,
        -0.5, -0.25, -1.0, -0.25, -0.75,
    ];
    assert_eq!(top_structures(&chances, 5), Ok(vec![1, 1, 0, 4, 0, 1]));
}

#[test]
fn no_network_is_opened_while_another_is_alive() {
    for robust in [false, true] {
        let mut shelf = Shelf::default();
        put_top(&mut shelf, robust, GRID, 3, &[1; 16]);
        shelf.put(Role::Cortex, |_| Ok(vec![map(vec![0; 16])]));
        shelf.put(Role::Scores, no_scores);
        let (found, _) = run(&plain_image(), &three(), wanted(robust, true), &mut shelf);
        found.expect("the image is labelled");
        let openings = if robust { 5 } else { 3 };
        assert_eq!(shelf.alive_at_opening, vec![0; openings], "{robust}");
        assert_eq!(
            shelf.alive(),
            0,
            "{robust}: none is kept once the result is given"
        );
    }
}
