//! Tests of what tidying refuses: what is not the chances of the lists' structures on a grid,
//! a way back that does not fit them, and a grid there is not the memory for. Whatever it is
//! given, `tidy` gives an error and does not panic.

use voxa_models::{Lists, RunError, Tensor};

use super::tests::{ROBUST, STANDARD, found, line, lists};
use super::tidy;
use crate::{Back, Found};

/// Holds that tidying refuses this with a failure, in a plain sentence that names no file,
/// whichever networks are said to have run.
fn is_refused(found: &Found, lists: &Lists, back: &Back, about: &str) {
    for wanted in [STANDARD, ROBUST] {
        let Err(RunError::Failed(words)) = tidy(found, lists, wanted, back) else {
            panic!("{about}: a failure was expected");
        };
        assert!(words.ends_with('.'), "{about}: {words}");
        assert!(!words.contains(['/', '\\', ':']), "{about}: {words}");
    }
}

#[test]
fn what_is_not_the_chances_of_the_lists_structures_is_a_failure() {
    let lists = lists(&[(0, 0), (7, 1), (9, 1)], &[], [3, 42]);
    let good = || found([4, 1, 1], 3, vec![0.25; 12], None);
    for wanted in [STANDARD, ROBUST] {
        tidy(&good(), &lists, wanted, &line(4)).expect("this much is tidied");
    }
    let two = found([6, 1, 1], 2, vec![0.25; 12], None);
    is_refused(&two, &lists, &line(6), "chances of two structures");
    let mut bytes = good();
    bytes.probabilities = Tensor::u8(vec![1, 4, 1, 1, 3], vec![0; 12]).expect("whole");
    is_refused(&bytes, &lists, &line(4), "bytes for chances");
    let mut flat = good();
    flat.probabilities.shape = vec![1, 4, 3];
    is_refused(&flat, &lists, &line(4), "no grid");
    let mut short = good();
    short.probabilities.shape = vec![1, 5, 1, 1, 3];
    is_refused(
        &short,
        &lists,
        &line(5),
        "fewer numbers than the shape says",
    );
    let none = found([4, 1, 1], 0, Vec::new(), None);
    let no_lists = super::tests::lists(&[], &[], [3, 42]);
    is_refused(&none, &no_lists, &line(4), "no structures at all");
    let mut areas = good();
    areas.areas = Some(Tensor::u8(vec![1, 2, 2, 1], vec![1; 4]).expect("whole"));
    is_refused(&areas, &lists, &line(4), "areas of another grid");
    areas.areas = Some(Tensor::f32(vec![1, 4, 1, 1], vec![1.0; 4]).expect("whole"));
    is_refused(&areas, &lists, &line(4), "areas that are not bytes");
}

#[test]
fn an_image_with_no_voxels_is_a_failure() {
    let lists = lists(&[(0, 0), (7, 1), (9, 1)], &[], [3, 42]);
    // A side of nought, each side in turn and all three: no chances at all, as the shape
    // says, and a way back that fits such an image.
    for size in [[0, 4, 1], [4, 0, 1], [4, 1, 0], [0, 0, 0]] {
        let nothing = found(size, 3, Vec::new(), None);
        let back = Back {
            size,
            grid_dims: size.map(|side| u32::try_from(side.max(1)).expect("small")),
            ..line(4)
        };
        is_refused(&nothing, &lists, &back, "an image with no voxels");
    }
}

#[test]
fn a_way_back_that_does_not_fit_what_was_found_is_a_failure() {
    let lists = lists(&[(0, 0), (7, 1), (9, 1)], &[], [3, 42]);
    let found = found([4, 1, 1], 3, vec![0.25; 12], None);
    let with = |change: fn(&mut Back)| {
        let mut back = line(4);
        change(&mut back);
        back
    };
    let longer = with(|back| back.size = [5, 1, 1]);
    is_refused(
        &found,
        &lists,
        &longer,
        "an image longer than the padded grid",
    );
    let padded = with(|back| back.padding_before = [1, 0, 0]);
    is_refused(&found, &lists, &padded, "padding that pushes the image out");
    let twice = with(|back| back.order = [0, 0, 2]);
    is_refused(&found, &lists, &twice, "an axis taken twice");
    let no_axis = with(|back| back.order = [0, 1, 3]);
    is_refused(&found, &lists, &no_axis, "an axis there is not");
    let cut = with(|back| back.cut_from = [1, 0, 0]);
    is_refused(&found, &lists, &cut, "a cut that ends past the grid");
    let small = with(|back| back.grid_dims = [3, 1, 1]);
    is_refused(&found, &lists, &small, "a grid shorter than the image");
    let far = with(|back| back.padding_before = [usize::MAX, 0, 0]);
    is_refused(&found, &lists, &far, "padding past all counting");
}

#[test]
fn a_grid_there_is_not_the_memory_for_is_no_room_and_does_not_end_the_program() {
    let lists = lists(&[(0, 0), (7, 1), (9, 1)], &[], [3, 42]);
    let found = found([4, 1, 1], 3, vec![0.25; 12], None);
    // A grid of a thousand million million voxels, of which the image shows four; and one
    // whose voxels cannot be counted.
    for dims in [[100_000; 3], [u32::MAX; 3]] {
        let mut vast = line(4);
        vast.grid_dims = dims;
        for wanted in [STANDARD, ROBUST] {
            let refused = tidy(&found, &lists, wanted, &vast);
            assert_eq!(refused, Err(RunError::NoRoom), "{dims:?}");
        }
    }
}
