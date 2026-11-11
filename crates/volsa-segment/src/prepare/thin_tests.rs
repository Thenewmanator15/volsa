//! Tests of the thin stacks that `SynthSeg`'s own program stops on, which Volsa therefore
//! refuses. No golden image can be made of a stack that program stops on. What it did with
//! each size here was run, with its own `preprocess`, and is said beside the size: it stops,
//! or it gives an image of the size the test expects.

use volsa_core::{Affine, Volume};
use volsa_models::Cut;

use super::PrepareError::{self, NothingToSee, TooFewSlices};
use super::prepare;

// Where a step leads in the patient, whose axes are left, back and top.
const RIGHT: [f64; 3] = [-1.0, 0.0, 0.0];
const FRONT: [f64; 3] = [0.0, -1.0, 0.0];
const TOP: [f64; 3] = [0.0, 0.0, 1.0];
/// Stored as `SynthSeg` wants a volume: columns to the right, rows to the front.
const STRAIGHT: Runs = [RIGHT, FRONT, TOP];
const ONE_MM: [f64; 3] = [1.0; 3];

/// Where the columns, the rows and the slices of a stack run.
type Runs = [[f64; 3]; 3];
/// The image's size along x, y and z before padding, or why the stack is refused.
type Made = Result<[usize; 3], PrepareError>;

/// What `prepare` makes of a stack whose columns, rows and slices run as `runs` says, `steps`
/// millimetres apart, each value made from its place in the list.
fn made_of(dims: [u32; 3], runs: Runs, steps: [f64; 3], value: fn(u32) -> f32) -> Made {
    let [across, down, through] = [0, 1, 2].map(|axis| runs[axis].map(|part| part * steps[axis]));
    let values = (0..dims.iter().product()).map(value).collect();
    let placement = Affine::from_columns(across, down, through, [0.0; 3]);
    let volume = Volume::new(dims, values, placement).expect("a volume");
    prepare(&volume, Cut::Whole).map(|prepared| prepared.back.size)
}

/// What `prepare` makes of a stack of values that differ.
fn made(dims: [u32; 3], runs: Runs, steps: [f64; 3]) -> Made {
    made_of(dims, runs, steps, |place| (place % 97) as f32)
}

#[test]
fn ten_slices_as_stored_are_too_few_and_eleven_are_enough() {
    // `SynthSeg` stops on the first of each pair and gives an image for the second.
    assert_eq!(made([20, 20, 10], STRAIGHT, ONE_MM), Err(TooFewSlices));
    assert_eq!(made([20, 20, 11], STRAIGHT, ONE_MM), Ok([20, 20, 11]));
    // Slices 5 mm apart: ten would be 50 at 1 mm, and it stops all the same.
    let thick = [1.0, 1.0, 5.0];
    assert_eq!(made([20, 20, 10], STRAIGHT, thick), Err(TooFewSlices));
    assert_eq!(made([20, 20, 11], STRAIGHT, thick), Ok([20, 20, 55]));
    // Slices running to the patient's right: it is the count as stored that it stops on.
    let sideways = [FRONT, TOP, RIGHT];
    assert_eq!(made([20, 20, 10], sideways, ONE_MM), Err(TooFewSlices));
    assert_eq!(made([20, 20, 11], sideways, ONE_MM), Ok([11, 20, 20]));
}

#[test]
fn ten_voxels_from_bottom_to_top_at_1_mm_are_too_few_and_eleven_are_enough() {
    // Rows running to the top: twenty slices as stored, but the image's z is the rows.
    // `SynthSeg` stops on ten rows and gives an image for eleven.
    let upright = [RIGHT, TOP, FRONT];
    assert_eq!(made([20, 10, 20], upright, ONE_MM), Err(TooFewSlices));
    assert_eq!(made([20, 11, 20], upright, ONE_MM), Ok([20, 20, 11]));
    // Slices half a millimetre apart: twenty are ten at 1 mm, where it stops; 22 are eleven.
    let fine = [1.0, 1.0, 0.5];
    assert_eq!(made([20, 20, 20], STRAIGHT, fine), Err(TooFewSlices));
    assert_eq!(made([20, 20, 22], STRAIGHT, fine), Ok([20, 20, 11]));
}

#[test]
fn a_short_side_across_or_from_front_to_back_is_allowed_down_to_four() {
    // Ten along x at 1 mm, where ten along z is refused: `SynthSeg` gives an image.
    let sideways = [FRONT, TOP, RIGHT];
    assert_eq!(
        made([20, 20, 20], sideways, [1.0, 1.0, 0.5]),
        Ok([10, 20, 20])
    );
    // And for these two.
    assert_eq!(made([5, 6, 12], STRAIGHT, ONE_MM), Ok([5, 6, 12]));
    assert_eq!(made([4, 4, 11], STRAIGHT, ONE_MM), Ok([4, 4, 11]));
    // Under four is Volsa's own refusal, as stored and at 1 mm. `SynthSeg` gives an image of
    // each of the two refused here: three columns 2 mm apart, which are six at 1 mm, and
    // twelve a quarter of a millimetre apart, which are three.
    let coarse = [2.0, 1.0, 1.0];
    assert_eq!(made([3, 20, 20], STRAIGHT, coarse), Err(TooFewSlices));
    assert_eq!(made([4, 20, 20], STRAIGHT, coarse), Ok([8, 20, 20]));
    let narrow = [0.25, 1.0, 1.0];
    assert_eq!(made([12, 20, 20], STRAIGHT, narrow), Err(TooFewSlices));
    assert_eq!(made([16, 20, 20], STRAIGHT, narrow), Ok([4, 20, 20]));
}

#[test]
fn which_side_runs_from_bottom_to_top_is_judged_at_1_mm() {
    // Tilted 40 degrees about x, the rows 3 mm apart and the slices half a millimetre. At
    // 1 mm the slices are the axis that runs from bottom to top. Judged from the transform as
    // stored, with its unequal steps, `SynthSeg`'s rule would pick the rows instead.
    let (sin, cos) = 40_f64.to_radians().sin_cos();
    let tilted = [RIGHT, [0.0, -cos, sin], [0.0, sin, cos]];
    let steps = [1.0, 3.0, 0.5];
    // Sixteen slices are eight from bottom to top at 1 mm, give or take one: `SynthSeg`
    // stops, though there are 36 voxels along the rows.
    assert_eq!(made([20, 12, 16], tilted, steps), Err(TooFewSlices));
    // Thirty are fifteen: it gives an image, the slices along z.
    let [x, y, z] = made([20, 12, 30], tilted, steps).expect("it prepares");
    assert!(
        x == 20 && y >= 36 && (15..=16).contains(&z),
        "{x} x {y} x {z}"
    );
}

#[test]
fn the_sizes_decide_before_any_value_is_read() {
    // Nothing in these stacks is a number, and the thin ones are still refused for their size.
    let empty = |dims: [u32; 3], runs: Runs| made_of(dims, runs, ONE_MM, |_| f32::NAN);
    assert_eq!(empty([20, 20, 10], STRAIGHT), Err(TooFewSlices));
    assert_eq!(empty([20, 10, 20], [RIGHT, TOP, FRONT]), Err(TooFewSlices));
    assert_eq!(empty([20, 20, 11], STRAIGHT), Err(NothingToSee));
}

#[test]
fn the_refusal_says_what_is_needed_and_that_it_is_counted_at_1_mm() {
    // Twelve slices a fifth of a millimetre apart: enough as stored, three at 1 mm. The
    // sentence has to be true of this stack as of one with three slices.
    assert_eq!(
        made([20, 20, 12], STRAIGHT, [1.0, 1.0, 0.2]),
        Err(TooFewSlices)
    );
    let said = TooFewSlices.to_string();
    for part in [
        "eleven",
        "as stored",
        "at 1 mm",
        "from bottom to top",
        "four",
    ] {
        assert!(said.contains(part), "no {part:?} in: {said}");
    }
    assert!(
        !said.contains("too few slices"),
        "not true of every stack: {said}"
    );
    assert!(
        said.ends_with('.') && !said.contains("::") && !said.contains('\n'),
        "{said}"
    );
}

/// A scan as it is held, of which only its sizes and its place may be asked: reading a value
/// is noted.
struct Unread {
    dims: [u32; 3],
    placement: Affine<volsa_core::Voxel, volsa_core::Patient>,
    read: std::sync::atomic::AtomicBool,
}

impl volsa_core::Grid for Unread {
    fn dims(&self) -> [u32; 3] {
        self.dims
    }

    fn voxel_to_patient(&self) -> Affine<volsa_core::Voxel, volsa_core::Patient> {
        self.placement
    }

    fn patient_to_voxel(&self) -> Affine<volsa_core::Patient, volsa_core::Voxel> {
        self.placement
            .inverse()
            .expect("a placement that can be undone")
    }

    fn value(&self, _col: u32, _row: u32, _slice: u32) -> Option<f32> {
        self.read.store(true, std::sync::atomic::Ordering::SeqCst);
        None
    }
}

#[test]
fn whether_a_scan_fits_is_known_from_its_sizes_without_reading_a_value_and_as_prepare_says() {
    use super::PrepareError::TooLarge;
    let asked = |dims: [u32; 3], runs: Runs, steps: [f64; 3]| {
        let [across, down, through] =
            [0, 1, 2].map(|axis| runs[axis].map(|part| part * steps[axis]));
        let scan = Unread {
            dims,
            placement: Affine::from_columns(across, down, through, [0.0; 3]),
            read: std::sync::atomic::AtomicBool::new(false),
        };
        let fits = super::fits(&scan);
        assert!(
            !scan.read.load(std::sync::atomic::Ordering::SeqCst),
            "no value is read"
        );
        fits
    };
    // A long CT, far too large to copy for a run that would refuse it: 3000 slices at 1 mm.
    assert_eq!(asked([512, 512, 3000], STRAIGHT, ONE_MM), Err(TooLarge));
    assert_eq!(
        asked([2600, 64, 64], STRAIGHT, [0.2, 1.0, 1.0]),
        Err(TooLarge)
    );
    // And for sizes small enough to prepare, the same answer as preparing gives.
    let sizes: [([u32; 3], [f64; 3]); 5] = [
        ([8, 8, 11], ONE_MM),
        ([8, 8, 10], ONE_MM),
        ([3, 8, 11], ONE_MM),
        ([8, 8, 4], [1.0, 1.0, 3.0]),
        ([16, 16, 11], [0.25, 0.25, 1.0]),
    ];
    for (dims, steps) in sizes {
        let prepared = made(dims, STRAIGHT, steps).map(|_| ());
        assert_eq!(asked(dims, STRAIGHT, steps), prepared, "{dims:?} {steps:?}");
    }
}
