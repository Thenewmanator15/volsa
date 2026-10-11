//! Tests of finding the largest piece of a mask. The grids are tiny and made up.

use crate::largest_piece;

/// A place in a grid, or its size: x, y, then z, which changes fastest in a mask.
type Triple = [usize; 3];

/// A mask of a grid of this size in which just these voxels are set.
fn mask_of(dims: Triple, lit: &[Triple]) -> Vec<bool> {
    let [_, deep, high] = dims;
    let mut mask = vec![false; dims.iter().product()];
    for &[x, y, z] in lit {
        mask[(x * deep + y) * high + z] = true;
    }
    mask
}

/// The voxels of the largest piece of the voxels lit, in the order of the mask.
fn kept(dims: Triple, lit: &[Triple]) -> Vec<Triple> {
    let [_, deep, high] = dims;
    let piece = largest_piece(&mask_of(dims, lit), dims);
    assert_eq!(piece.len(), dims.iter().product::<usize>());
    let places = piece.iter().enumerate().filter(|&(_, &set)| set);
    places
        .map(|(place, _)| [place / (deep * high), place / high % deep, place % high])
        .collect()
}

#[test]
fn of_five_voxels_and_three_that_touch_them_at_a_corner_the_five_are_kept() {
    let five: Vec<Triple> = (0..5).map(|z| [1, 1, z]).collect();
    // The first of the three is one step from the last of the five along every axis.
    let three = [[2, 2, 5], [3, 2, 5], [3, 3, 5]];
    let lit = [five.clone(), three.to_vec()].concat();
    assert_eq!(kept([4, 4, 6], &lit), five);
    // Along an edge is not joined either: one step along two axes.
    let two = [[2, 2, 4], [3, 2, 4]];
    let lit = [five.clone(), two.to_vec()].concat();
    assert_eq!(kept([4, 4, 6], &lit), five);
    // Across a face is: one step along one axis, and all seven are one piece.
    let beside = [[2, 1, 4], [3, 1, 4]];
    let lit = [five, beside.to_vec()].concat();
    assert_eq!(kept([4, 4, 6], &lit).len(), 7);
}

#[test]
fn of_two_pieces_as_large_the_first_met_is_kept() {
    // The piece at the lower x is met first, though the other begins at a lower y and z.
    let early = [[0, 3, 4], [0, 3, 5]];
    let late = [[2, 0, 0], [2, 0, 1]];
    assert_eq!(kept([4, 4, 6], &[late, early].concat()), early);
    // At one x, the lower y is met first, and at one y the lower z.
    let (low_y, high_y) = ([[1, 0, 5]], [[1, 2, 0]]);
    assert_eq!(kept([4, 4, 6], &[high_y, low_y].concat()), low_y);
    let (low_z, high_z) = ([[1, 2, 1]], [[1, 2, 3]]);
    assert_eq!(kept([4, 4, 6], &[high_z, low_z].concat()), low_z);
    // A later piece that is larger still wins.
    let three = [[3, 1, 1], [3, 1, 2], [3, 2, 2]];
    assert_eq!(
        kept([4, 4, 6], &[early.to_vec(), three.to_vec()].concat()),
        three
    );
}

#[test]
fn of_nothing_nothing_is_kept() {
    assert_eq!(kept([4, 4, 6], &[]), Vec::<Triple>::new());
    assert_eq!(largest_piece(&[], [0, 0, 0]), Vec::<bool>::new());
    assert_eq!(largest_piece(&[], [4, 0, 6]), Vec::<bool>::new());
}

#[test]
fn a_piece_is_followed_along_every_axis_and_not_round_the_end_of_a_line() {
    // The last voxel of one line along z and the first of the next lie side by side in the
    // mask and far apart in the grid: they are two pieces, and the first is kept.
    assert_eq!(kept([2, 3, 4], &[[0, 0, 3], [0, 1, 0]]), [[0, 0, 3]]);
    // So with the last line of one x and the first line of the next.
    assert_eq!(kept([2, 3, 4], &[[0, 2, 3], [1, 0, 0]]), [[0, 2, 3]]);
    // A piece that turns along z, y and x in turn, both ways, is one piece.
    let winding = [
        [0, 0, 0],
        [0, 0, 1],
        [0, 1, 1],
        [1, 1, 1],
        [1, 1, 0],
        [1, 0, 0],
    ];
    assert_eq!(kept([2, 3, 4], &winding).len(), 6);
    // A whole grid is one piece.
    assert_eq!(largest_piece(&[true; 24], [2, 3, 4]), [true; 24]);
}

#[test]
fn a_mask_that_is_not_of_the_grid_is_given_back_as_it_is() {
    let mask = [true, false, true, false, true];
    assert_eq!(largest_piece(&mask, [2, 3, 4]), mask);
    assert_eq!(largest_piece(&mask, [usize::MAX, 2, 2]), mask);
}
