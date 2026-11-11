//! Tests of cutting to the head. The volumes are made up: faint noise with balls and blocks of
//! brightness in it, so the head is wherever a test puts it.

use voxa_core::{Affine, Patient, Volume, Voxel};
use voxa_models::{Cut, Numbers};

use super::PrepareError::{NothingToSee, TooFewSlices, TooLarge};
use super::align::Turned;
use super::head::{Block, Piece, Sizes, head_block, largest_piece, sizes};
use super::{Prepared, prepare};

type Placement = Affine<Voxel, Patient>;
/// A place in a grid, or a count of voxels along each axis.
type Triple = [usize; 3];

/// The side of the cube most volumes here are.
const SIDE: u32 = 128;
/// The brightness of the head.
const BRIGHT: f32 = 100.0;
/// The noise is this and 1 and 2 more, so the dimmest voxels are this bright.
const FLOOR: f32 = 40.0;
/// The voxels in a cube of 128 each way.
const CUBED: u64 = 128 * 128 * 128;

/// Something bright in a volume.
enum Shape {
    /// Every voxel within `radius` of `at`.
    Ball { at: [u32; 3], radius: u32 },
    /// Every voxel from `from` on, so many along each axis.
    Block { from: [u32; 3], size: [u32; 3] },
}

impl Shape {
    fn holds(&self, voxel: [u32; 3]) -> bool {
        match *self {
            Self::Ball { at, radius } => {
                let apart = voxel.iter().zip(&at).map(|(&a, &b)| a.abs_diff(b).pow(2));
                apart.sum::<u32>() <= radius * radius
            }
            Self::Block { from, size } => voxel
                .iter()
                .zip(from.iter().zip(&size))
                .all(|(&place, (&start, &long))| (start..start + long).contains(&place)),
        }
    }
}

fn ball(at: [u32; 3]) -> (Shape, f32) {
    (Shape::Ball { at, radius: 30 }, BRIGHT)
}

fn block(from: [u32; 3], size: [u32; 3], brightness: f32) -> (Shape, f32) {
    (Shape::Block { from, size }, brightness)
}

/// A pattern of 0, 1 and 2 more than the floor, dim enough to lie below any mark a head sets.
fn faint([col, row, slice]: [u32; 3]) -> f32 {
    FLOOR + ((col + 2 * row + 2 * slice) % 3) as f32
}

/// Faint noise with these shapes in it: the first shape that holds a voxel sets its brightness.
fn noise_with(shapes: &[(Shape, f32)]) -> impl Fn([u32; 3]) -> f32 + '_ {
    move |voxel| {
        let held = shapes.iter().find(|(shape, _)| shape.holds(voxel));
        held.map_or_else(|| faint(voxel), |&(_, brightness)| brightness)
    }
}

/// Columns to the patient's right, rows to the front and slices to the top, `step` mm apart.
fn straight(step: f64) -> Placement {
    let columns = [-step, 0.0, 0.0];
    Affine::from_columns(columns, [0.0, -step, 0.0], [0.0, 0.0, step], [0.0; 3])
}

/// A volume whose value at each [column, row, slice] is `value` of it.
fn made(dims: [u32; 3], placement: Placement, value: impl Fn([u32; 3]) -> f32) -> Volume {
    let [cols, rows, slices] = dims;
    let mut values = Vec::new();
    for slice in 0..slices {
        for row in 0..rows {
            values.extend((0..cols).map(|col| value([col, row, slice])));
        }
    }
    Volume::new(dims, values, placement).expect("a volume")
}

fn headed(volume: &Volume) -> Prepared {
    prepare(volume, Cut::Head).expect("it prepares")
}

/// A cube of faint noise with these shapes in it, cut to the head.
fn headed_cube(shapes: &[(Shape, f32)], placement: Placement) -> Prepared {
    headed(&made([SIDE; 3], placement, noise_with(shapes)))
}

/// Where the cut began in the turned grid, and how many voxels it has along each axis.
fn cut_of(prepared: &Prepared) -> (Triple, Triple) {
    (prepared.back.cut_from, prepared.back.size)
}

/// The image's value at [x, y, z], counted from where the padding ends.
fn at(prepared: &Prepared, [x, y, z]: Triple) -> f32 {
    let [before_x, before_y, before_z] = prepared.back.padding_before;
    let shape = &prepared.image.shape;
    let Numbers::F32(numbers) = &prepared.image.numbers else {
        panic!("an image is floating point")
    };
    numbers[((x + before_x) * shape[2] + y + before_y) * shape[3] + z + before_z]
}

fn small(place: Triple) -> [u32; 3] {
    place.map(|count| u32::try_from(count).expect("small"))
}

/// Holds the image against `lit`: the image is bright at a place of the turned grid just when
/// `lit` says that place is. The place is the image's, plus where the cut began.
fn image_shows(prepared: &Prepared, lit: impl Fn(Triple) -> bool) {
    let [from_x, from_y, from_z] = prepared.back.cut_from;
    let [wide, deep, high] = prepared.back.size;
    for x in 0..wide {
        for y in 0..deep {
            for z in 0..high {
                let place = [x + from_x, y + from_y, z + from_z];
                assert_eq!(at(prepared, [x, y, z]) > 0.5, lit(place), "{place:?}");
            }
        }
    }
}

#[test]
fn a_bright_ball_is_cut_out_with_sixteen_voxels_all_round() {
    let volume = made([SIDE; 3], straight(1.0), noise_with(&[ball([64; 3])]));
    let prepared = headed(&volume);
    // The ball runs from 34 to 94 along every axis, and 16 more each side is 18 to 110.
    assert_eq!(cut_of(&prepared), ([18; 3], [93; 3]));
    // Each side of 93 is padded to 128, as a whole side of 128 is.
    assert_eq!(prepared.image.shape, [1, 128, 128, 128, 1]);
    let both = Sizes {
        whole: CUBED,
        head: CUBED,
    };
    assert_eq!(sizes(&volume), Ok(both));
}

#[test]
fn the_box_is_kept_within_the_grid_and_the_image_shows_the_ball_where_it_was() {
    let shapes = [ball([32, 64, 96])];
    let prepared = headed_cube(&shapes, straight(1.0));
    // The ball runs 2 to 62 along x, 34 to 94 along y and 66 to 126 along z. Sixteen more
    // before it would pass the start along x, and sixteen after it would pass the end along z.
    assert_eq!(cut_of(&prepared), ([0, 18, 50], [79, 93, 78]));
    image_shows(&prepared, |place| shapes[0].0.holds(small(place)));
}

#[test]
fn the_cut_is_in_the_turned_grid_whatever_the_axes_of_the_volume() {
    // Columns run to the patient's back, rows down and slices to the left: the image's x runs
    // along the slices, its y along the columns and its z along the rows, each the other way.
    let sagittal =
        Affine::from_columns([0.0, 1.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0; 3]);
    // The ball's centre is [80, 64, 96] in the image, so [127 - 64, 127 - 96, 127 - 80] as stored.
    let shapes = [ball([63, 31, 47])];
    let prepared = headed_cube(&shapes, sagittal);
    let back = &prepared.back;
    assert_eq!((back.order, back.turned_round), ([2, 0, 1], [true; 3]));
    assert_eq!(cut_of(&prepared), ([34, 18, 50], [93, 93, 78]));
    // The voxel of the volume that an image place shows, by what `Back` says.
    let stored = |place: Triple| {
        let mut voxel = [0; 3];
        for axis in 0..3 {
            let far = SIDE as usize - 1;
            let along = place[axis];
            voxel[back.order[axis]] = if back.turned_round[axis] {
                far - along
            } else {
                along
            };
        }
        small(voxel)
    };
    image_shows(&prepared, |place| shapes[0].0.holds(stored(place)));
}

#[test]
fn a_ball_in_a_larger_cube_is_a_much_smaller_head() {
    let volume = made([256; 3], straight(1.0), noise_with(&[ball([128; 3])]));
    let whole = 256 * 256 * 256;
    assert_eq!(sizes(&volume), Ok(Sizes { whole, head: CUBED }));
}

#[test]
fn a_speck_of_three_voxels_far_from_the_ball_does_not_widen_the_box() {
    let speck = block([4, 4, 4], [3, 1, 1], BRIGHT);
    let prepared = headed_cube(&[ball([64; 3]), speck], straight(1.0));
    assert_eq!(cut_of(&prepared), ([18; 3], [93; 3]));
}

#[test]
fn a_voxel_is_head_when_a_tenth_of_the_way_from_the_dimmest_to_the_brightest() {
    // The dimmest is 40 and the brightest 100, so the mark is 46. A block larger than the
    // ball is the head when it is 46.25, and is left out when it is 45.75.
    let found = |brightness: f32| {
        let shapes = [ball([32, 64, 64]), block([70, 34, 34], [52; 3], brightness)];
        cut_of(&headed_cube(&shapes, straight(1.0)))
    };
    assert_eq!(found(45.75), ([0, 18, 18], [79, 93, 93]), "the ball");
    assert_eq!(found(46.25), ([54, 18, 18], [74, 84, 84]), "the block");
}

#[test]
fn the_mark_comes_from_the_whole_grid_and_the_brightness_from_the_cut() {
    // A slab twice as bright as the ball, smaller than it and off to one side, sets the high
    // percentile of the whole grid, so the mark is 56. It lies outside the cut, where the high
    // percentile is the ball's. A block larger than the ball, also outside the cut, is brighter
    // than 46, the mark if the ball's 100 were the high percentile, and dimmer than 56: it is
    // not the head.
    let slab = block([0, 0, 0], [12, SIDE, 40], 2.0 * BRIGHT);
    let dim = block([111, 0, 0], [17, SIDE, SIDE], 50.0);
    let prepared = headed_cube(&[slab, dim, ball([64; 3])], straight(1.0));
    assert_eq!(cut_of(&prepared), ([18; 3], [93; 3]));
    assert_eq!(at(&prepared, [46; 3]), 1.0, "the ball's centre");
    assert!(at(&prepared, [0; 3]) < 0.05, "the noise at a corner");
}

#[test]
fn nothing_above_the_mark_is_the_whole_grid() {
    let size = [12, 16, 20];
    let turned = Turned {
        size,
        values: vec![1.0; 12 * 16 * 20],
    };
    assert_eq!(largest_piece(&turned, 2.0), None);
    assert_eq!(head_block(&turned, 2.0), Block { from: [0; 3], size });
}

#[test]
fn a_piece_that_fills_the_grid_is_the_whole_grid() {
    // Noise alone: most voxels lie above the mark, and join up to take the whole grid.
    let prepared = headed_cube(&[], straight(1.0));
    assert_eq!(cut_of(&prepared), ([0; 3], [SIDE as usize; 3]));
}

/// The piece that `largest_piece` finds among the voxels lit, in a grid of `side` each way.
fn piece_of(side: usize, lit: &[Triple]) -> Option<Piece> {
    let mut values = vec![0.0; side * side * side];
    for &[x, y, z] in lit {
        values[(x * side + y) * side + z] = 1.0;
    }
    let turned = Turned {
        size: [side; 3],
        values,
    };
    largest_piece(&turned, 0.5)
}

#[test]
fn pieces_are_joined_by_their_faces_and_not_by_their_edges() {
    // A line of twelve voxels that meet along edges only, and a block of eight.
    let mut lit: Vec<Triple> = (0..12).map(|i| [i, i, 0]).collect();
    for x in 8..10 {
        for y in 2..4 {
            for z in 6..8 {
                lit.push([x, y, z]);
            }
        }
    }
    let found = piece_of(12, &lit);
    let block = Piece {
        count: 8,
        least: [8, 2, 6],
        most: [9, 3, 7],
    };
    assert_eq!(found, Some(block));
}

#[test]
fn a_winding_piece_is_followed_in_all_six_directions() {
    // Nodes two voxels apart, on a path that steps along +x, +y, -x, +z, +y, -z, +x, +x and -y.
    // One voxel joins each node to the next, and no other two voxels touch.
    let nodes = [
        [0, 0, 0],
        [1, 0, 0],
        [1, 1, 0],
        [0, 1, 0],
        [0, 1, 1],
        [0, 2, 1],
        [0, 2, 0],
        [1, 2, 0],
        [2, 2, 0],
        [2, 1, 0],
    ];
    let mut lit: Vec<Triple> = nodes.iter().map(|node| node.map(|n| 2 * n)).collect();
    for pair in nodes.windows(2) {
        let ([ax, ay, az], [bx, by, bz]) = (pair[0], pair[1]);
        lit.push([ax + bx, ay + by, az + bz]);
    }
    let found = piece_of(6, &lit);
    let winding = Piece {
        count: 19,
        least: [0, 0, 0],
        most: [4, 4, 2],
    };
    assert_eq!(found, Some(winding));
}

#[test]
fn a_piece_of_half_a_million_voxels_is_followed_to_its_end() {
    let turned = Turned {
        size: [80; 3],
        values: vec![1.0; 80 * 80 * 80],
    };
    let found = largest_piece(&turned, 0.5);
    let solid = Piece {
        count: 80 * 80 * 80,
        least: [0; 3],
        most: [79; 3],
    };
    assert_eq!(found, Some(solid));
}

#[test]
fn sizes_are_those_of_the_images_prepare_makes_even_when_it_resamples() {
    // 80 voxels each way, 1.75 mm apart, are 140 at 1 mm, padded to 160. The ball is not.
    let ball = Shape::Ball {
        at: [40; 3],
        radius: 14,
    };
    let volume = made([80; 3], straight(1.75), noise_with(&[(ball, BRIGHT)]));
    let cubed = |side: u64| side * side * side;
    let both = Sizes {
        whole: cubed(160),
        head: cubed(128),
    };
    assert_eq!(sizes(&volume), Ok(both));
    let whole = prepare(&volume, Cut::Whole).expect("it prepares");
    assert_eq!(whole.image.shape, [1, 160, 160, 160, 1]);
    assert_eq!(headed(&volume).image.shape, [1, 128, 128, 128, 1]);
}

#[test]
fn sizes_refuses_what_prepare_refuses_and_in_the_same_words() {
    // Every bright voxel stands alone, so the first of them is taken for the head, and the
    // cut round it holds nothing that differs. Cut whole, the same volume is made.
    let alone = |[col, row, slice]: [u32; 3]| {
        let apart = col >= 20 && col % 2 + row % 2 + slice % 2 == 0;
        if apart || col + row + slice == 0 {
            BRIGHT
        } else {
            0.0
        }
    };
    let lonely = made([80; 3], straight(1.0), alone);
    prepare(&lonely, Cut::Whole).expect("it prepares");
    let thin = made([20, 20, 10], straight(1.0), faint);
    let large = made([600, 8, 11], straight(1.0), faint);
    let same = made([20, 20, 20], straight(1.0), |_| 7.0);
    let refused = [
        (thin, TooFewSlices),
        (large, TooLarge),
        (same, NothingToSee),
        (lonely, NothingToSee),
    ];
    for (volume, refusal) in refused {
        assert_eq!(prepare(&volume, Cut::Head).expect_err("refused"), refusal);
        assert_eq!(sizes(&volume), Err(refusal));
    }
}
