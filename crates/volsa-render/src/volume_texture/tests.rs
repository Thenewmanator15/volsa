use super::*;
use crate::prepared::{as_halves, with_neighbours};

/// 16-bit floating-point 0, a half and 1, as stored.
const ZERO: u16 = 0x0000;
const HALF: u16 = 0x3800;
const ONE: u16 = 0x3C00;

fn row_of(values: &[f32]) -> Volume {
    let count = u32::try_from(values.len()).expect("a few");
    Volume::new([count, 1, 1], values.to_vec(), Affine::identity()).expect("adds up")
}

fn cube(side: u32) -> Volume {
    let values = vec![0.0; (side * side * side) as usize];
    Volume::new([side; 3], values, Affine::identity()).expect("adds up")
}

#[test]
fn values_are_stored_from_zero_at_the_smallest_to_one_at_the_largest() {
    let (stored, smallest, span) = as_halves(&row_of(&[10.0, 20.0, 30.0, -10.0]));
    assert_eq!(
        stored,
        [HALF, 0x3A00, ONE, ZERO],
        "a half, three quarters, one, nought"
    );
    assert_eq!((smallest, span), (-10.0, 40.0));
}

#[test]
fn values_that_are_not_numbers_are_stored_at_an_end() {
    let odd = [0.0, 10.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
    let (stored, smallest, span) = as_halves(&row_of(&odd));
    assert_eq!(stored, [ZERO, ONE, ZERO, ONE, ZERO]);
    assert_eq!(
        (smallest, span),
        (0.0, 10.0),
        "the range is that of the numbers alone"
    );
}

#[test]
fn a_volume_of_one_value_or_of_no_numbers_is_stored_as_zeros() {
    assert_eq!(
        as_halves(&row_of(&[5.0, 5.0])),
        (vec![ZERO, ZERO], 5.0, 1.0)
    );
    assert_eq!(
        as_halves(&row_of(&[f32::NAN, f32::NAN])),
        (vec![ZERO, ZERO], 0.0, 1.0)
    );
}

#[test]
fn a_block_takes_in_the_largest_value_of_the_blocks_around_it() {
    assert_eq!(
        with_neighbours(&[0.0, 0.0, 5.0, 0.0, 0.0], [5, 1, 1]),
        [0.0, 5.0, 5.0, 5.0, 0.0]
    );
    // A spot in one corner of 3 x 3 x 2 blocks reaches the 2 x 2 x 2 blocks about it.
    let mut spot = vec![0.0; 18];
    spot[0] = 7.0;
    let spread = with_neighbours(&spot, [3, 3, 2]);
    let reached: Vec<usize> = (0..18).filter(|at| spread[*at] > 0.0).collect();
    assert_eq!(reached, [0, 1, 3, 4, 9, 10, 12, 13]);
}

#[test]
fn spreading_does_not_run_on_from_the_end_of_one_row_into_the_next() {
    // 3 columns by 2 rows: the spot begins the second row, next in memory to the end of
    // the first.
    let spread = with_neighbours(&[0.0, 0.0, 0.0, 5.0, 0.0, 0.0], [3, 2, 1]);
    assert_eq!(spread, [5.0, 5.0, 0.0, 5.0, 5.0, 0.0]);
    let lone = with_neighbours(&[f32::NEG_INFINITY, 2.0], [2, 1, 1]);
    assert_eq!(
        lone,
        [2.0, 2.0],
        "a block of no numbers takes its neighbour's"
    );
}

#[test]
fn a_volume_is_halved_as_often_as_asked_before_it_is_placed() {
    let whole = placed_by_halving(&cube(8), 0, |shown| Some(shown.dims()));
    assert_eq!(whole, Some(([8, 8, 8], 0)));
    let twice = placed_by_halving(&cube(8), 2, |shown| Some(shown.dims()));
    assert_eq!(twice, Some(([2, 2, 2], 2)));
}

#[test]
fn a_volume_there_is_no_room_for_is_halved_until_there_is() {
    let room_for = |side: u32| {
        move |shown: &Volume| {
            shown
                .dims()
                .into_iter()
                .all(|count| count <= side)
                .then_some(())
        }
    };
    assert_eq!(placed_by_halving(&cube(8), 0, room_for(2)), Some(((), 2)));
    assert_eq!(placed_by_halving(&cube(8), 1, room_for(4)), Some(((), 1)));
    assert_eq!(
        placed_by_halving(&cube(8), 0, room_for(1)),
        Some(((), 3)),
        "one voxel"
    );
}

#[test]
fn with_no_room_even_for_one_voxel_nothing_is_placed() {
    let mut tries = 0;
    let placed = placed_by_halving(&cube(8), 0, |_| {
        tries += 1;
        None::<()>
    });
    assert_eq!((placed, tries), (None, 4), "at 8, 4, 2 and 1 voxels a side");
}
