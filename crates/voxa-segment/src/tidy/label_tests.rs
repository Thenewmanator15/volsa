//! Tests of taking a volume all the way to labels. Stand-ins answer for the networks: each is
//! sure of one structure at every voxel, by where the voxel lies in the grid it is given. The
//! volumes are small and made up, and no network is read or run.

use voxa_core::{Affine, Patient, Point, Volume, Voxel};
use voxa_labels::Side;
use voxa_models::testkit::Canned;
use voxa_models::{Cut, Lists, ModelSide, Network, Role, RunError, Structure, Tensor, Wanted};

use crate::label::notes;
use crate::{
    LabelError, Labelled, ModelChoice, Networks, PrepareError, Stage, THICK_MM, choose, label,
};

/// The label of the structure on the left in the lists of these tests, and of its twin.
const LEFT: u32 = 17;
const RIGHT: u32 = 53;
/// The robust networks, without the cortex; and the standard one.
const ROBUST: Wanted = Wanted {
    robust: true,
    cortex: false,
};
const STANDARD: Wanted = Wanted {
    robust: false,
    cortex: false,
};
const ROUGH_AT_6: &str = "The slices of this scan are 6 mm apart, so this is a rough map.";
const CUT: &str = "The scan was cut to the head to fit. Labels at the very edge may differ.";

/// Which structure a stand-in is sure of at a voxel: its place among the three of the lists,
/// from the voxel's place and the size of the grid, each along x, y and z.
type Top = fn([usize; 3], [usize; 3]) -> u8;

/// Nothing, and one structure on each side with a class of its own; two score groups.
fn lists() -> Lists {
    let part = |label: u32, name: &str, side: ModelSide, class: u32| Structure {
        label,
        name: name.to_owned(),
        side,
        class,
    };
    Lists {
        structures: vec![
            part(0, "background", ModelSide::Midline, 0),
            part(LEFT, "hippocampus", ModelSide::Left, 1),
            part(RIGHT, "hippocampus", ModelSide::Right, 2),
        ],
        flip: vec![0, 2, 1],
        stage_classes: 2,
        areas: Vec::new(),
        cortex: [3, 42],
        score_class: vec![0, 1, 2],
        score_groups: vec!["one".to_owned(), "two".to_owned()],
        box_side: 8,
    }
}

/// Stand-ins for the networks. Every one answers as it should; the one that gives the chances
/// is sure of the structure `top` names at each voxel.
struct Stand {
    top: Top,
    /// What a network says when it is asked to open, if it is not to open.
    shut: Option<RunError>,
}

impl Networks for Stand {
    fn open(&mut self, role: Role, sizes: [usize; 3]) -> Result<Box<dyn Network>, RunError> {
        if let Some(why) = &self.shut {
            return Err(why.clone());
        }
        let [x, y, z] = sizes;
        let top = self.top;
        let mut runs = 0;
        let answer = move |_: &[Tensor]| {
            runs += 1;
            Ok(match role {
                // The standard network runs a second time, on the mirror. There it answers with
                // no chance of anything, so the mean of its two answers is half the first.
                Role::Standard if runs == 2 => chances(|_, _| u8::MAX, sizes),
                Role::Standard | Role::Robust3 => chances(top, sizes),
                Role::Robust1 | Role::Robust2 | Role::Cortex => {
                    vec![Tensor::u8(vec![1, x, y, z], vec![1; x * y * z]).expect("a map")]
                }
                // The first number is dropped; the second group fails.
                Role::Scores => vec![Tensor::f32(vec![1, 3], vec![0.0, 0.9, 0.58]).expect("whole")],
            })
        };
        Ok(Box::new(Canned::new(answer)))
    }
}

/// The chances of the three structures at every voxel of a grid, as a network gives them: in
/// two tensors, the first with two structures. Each voxel is sure of the one `top` names.
fn chances(top: Top, size: [usize; 3]) -> Vec<Tensor> {
    let [x, y, z] = size;
    let (mut first, mut second) = (Vec::new(), Vec::new());
    for at_x in 0..x {
        for at_y in 0..y {
            for at_z in 0..z {
                let place = top([at_x, at_y, at_z], size);
                let sure = |of: u8| f32::from(u8::from(place == of));
                first.extend([sure(0), sure(1)]);
                second.push(sure(2));
            }
        }
    }
    vec![
        Tensor::f32(vec![1, x, y, z, 2], first).expect("the first half"),
        Tensor::f32(vec![1, x, y, z, 1], second).expect("the second half"),
    ]
}

/// The left structure in the half of the grid at low x, and nothing in the other half. In
/// the image x runs to the patient's right, so that is where a network finds the left side.
fn left_at_low_x([x, _, _]: [usize; 3], [wide, _, _]: [usize; 3]) -> u8 {
    u8::from(2 * x < wide)
}

/// A volume whose value at each column, row and slice is `value` of them.
fn volume_of(dims: [u32; 3], at: Affine<Voxel, Patient>, value: fn([u32; 3]) -> f32) -> Volume {
    let [cols, rows, slices] = dims;
    let mut values = Vec::new();
    for slice in 0..slices {
        for row in 0..rows {
            values.extend((0..cols).map(|col| value([col, row, slice])));
        }
    }
    Volume::new(dims, values, at).expect("a volume")
}

/// A value that rises along every axis.
fn ramp([col, row, slice]: [u32; 3]) -> f32 {
    (10 + col + 8 * row + 64 * slice) as f32
}

/// Columns to the patient's left, rows to the back and slices to the top, as a scanner stores
/// axial slices, so many millimetres apart.
fn axial(steps: [f64; 3]) -> Affine<Voxel, Patient> {
    let [across, down, through] = steps;
    let columns = [across, 0.0, 0.0];
    Affine::from_columns(
        columns,
        [0.0, down, 0.0],
        [0.0, 0.0, through],
        [-9.75, 6.25, 8.0],
    )
}

/// What `label` makes of a volume with the stand-ins, and the stages it says on the way.
fn run(volume: &Volume, cut: Cut, top: Top) -> (Result<Labelled, LabelError>, Vec<Stage>) {
    let mut networks = Stand { top, shut: None };
    let mut stages = Vec::new();
    let mut said = |stage| stages.push(stage);
    let labelled = label(volume, cut, &lists(), ROBUST, &mut networks, &mut said);
    (labelled, stages)
}

fn labelled(volume: &Volume, cut: Cut) -> Labelled {
    run(volume, cut, left_at_low_x).0.expect("it is labelled")
}

#[test]
fn a_structure_at_low_x_comes_back_on_the_patients_left_however_the_volume_is_stored() {
    let [left, back, top] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let down = [0.0, 0.0, -1.0];
    // Where the columns, the rows and the slices of an axial and of a sagittal series run.
    for [columns, rows, slices] in [[left, back, top], [back, down, left]] {
        let placement = Affine::from_columns(columns, rows, slices, [-9.75, 6.25, 8.0]);
        let volume = volume_of([20, 12, 16], placement, ramp);
        let labelled = labelled(&volume, Cut::Whole);
        let grid = &labelled.grid;
        assert_eq!((grid.dims, grid.placement), ([20, 12, 16], placement));
        assert_eq!(grid.ids.len(), 20 * 12 * 16);
        // The patient's x runs to the left: the labelled half is the one beyond the middle.
        let middle = volume.centre().x;
        for (place, &id) in grid.ids.iter().enumerate() {
            let [col, row, slice] = [place % 20, place / 20 % 12, place / 240].map(|n| n as f64);
            let on_the_left = placement.apply(Point::new(col, row, slice)).x > middle;
            let expected = if on_the_left { LEFT } else { 0 };
            assert_eq!(
                id, expected,
                "{columns:?}: column {col}, row {row}, slice {slice}"
            );
        }
        let named = |label: u32| labelled.table.entries.iter().find(|e| e.ids == [label]);
        assert_eq!(named(LEFT).map(|entry| entry.side), Some(Side::Left));
        assert_eq!(named(RIGHT).map(|entry| entry.side), Some(Side::Right));
        assert_eq!(labelled.notes, Vec::<String>::new());
    }
}

#[test]
fn the_stages_are_said_in_order_and_the_table_carries_the_models_scores() {
    let volume = volume_of([20, 12, 16], axial([1.0; 3]), ramp);
    let (labelled, stages) = run(&volume, Cut::Whole, left_at_low_x);
    let finding = |step| Stage::Finding { step, of: 3 };
    let said = [
        Stage::Preparing,
        finding(1),
        finding(2),
        finding(3),
        Stage::Checking,
        Stage::Tidying,
    ];
    assert_eq!(stages, said);
    let table = labelled.expect("it is labelled").table;
    assert_eq!(table.title, "SynthSeg 2.0, robust model");
    let verdict = table.verdict.expect("a verdict");
    assert!(!verdict.passed);
    assert_eq!(verdict.low, [("two".to_owned(), 0.58)]);
}

#[test]
fn nothing_but_background_is_nothing_found() {
    let volume = volume_of([20, 12, 16], axial([1.0; 3]), ramp);
    let (labelled, stages) = run(&volume, Cut::Whole, |_, _| 0);
    assert_eq!(labelled, Err(LabelError::NothingFound));
    assert_eq!(stages.last(), Some(&Stage::Tidying), "it was tidied first");
    // A structure found only in the padding round the image leaves no voxel of the scan
    // labelled: the image begins 54 voxels in along x.
    let in_the_padding: Top = |[x, _, _], _| u8::from(x < 50);
    let (labelled, _) = run(&volume, Cut::Whole, in_the_padding);
    assert_eq!(labelled, Err(LabelError::NothingFound));
    // One voxel labelled is something found.
    let one_voxel: Top = |at, _| u8::from(at == [54, 58, 56]);
    let found = run(&volume, Cut::Whole, one_voxel).0.expect("a voxel");
    assert_eq!(found.grid.ids.iter().filter(|&&id| id == LEFT).count(), 1);
}

#[test]
fn slices_six_millimetres_apart_make_a_rough_map() {
    let volume = volume_of([20, 12, 16], axial([1.0, 1.0, 6.0]), ramp);
    assert_eq!(labelled(&volume, Cut::Whole).notes, [ROUGH_AT_6]);
    // The largest of the three is named, to one decimal place at most; 3 mm is not rough.
    let rough = |spacing| notes(spacing, false).join(" ");
    assert_eq!(rough([6.0, 1.0, 1.0]), ROUGH_AT_6);
    assert_eq!(
        rough([1.0, 1.0, 3.5]),
        "The slices of this scan are 3.5 mm apart, so this is a rough map."
    );
    assert_eq!(
        rough([1.25, 8.0, 3.5]),
        "The slices of this scan are 8 mm apart, so this is a rough map."
    );
    assert_eq!(
        rough([0.75, 0.75, 3.26]),
        "The slices of this scan are 3.3 mm apart, so this is a rough map."
    );
    assert_eq!(rough([1.0, 1.0, 3.0]), "");
    assert_eq!(rough([1.0; 3]), "");
}

/// Faint noise with a bright ball of 6 voxels' radius in the middle of a cube of 48.
fn ball([col, row, slice]: [u32; 3]) -> f32 {
    let apart = [col, row, slice].map(|place| place.abs_diff(24).pow(2));
    if apart.iter().sum::<u32>() <= 36 {
        100.0
    } else {
        40.0 + ((col + 2 * row + 2 * slice) % 3) as f32
    }
}

#[test]
fn a_cut_to_the_head_is_noted_when_it_took_something_away() {
    // The ball and 16 voxels all round it are 45 of the 48 voxels each way.
    let small_head = volume_of([48; 3], axial([1.0; 3]), ball);
    assert_eq!(labelled(&small_head, Cut::Head).notes, [CUT]);
    // Here the head fills the grid, so nothing was cut away though a cut was asked for.
    let all_head = volume_of([20, 12, 16], axial([1.0; 3]), ramp);
    assert_eq!(labelled(&all_head, Cut::Head).notes, Vec::<String>::new());
    // The rough map is said first.
    assert_eq!(notes([1.0, 1.0, 6.0], true), [ROUGH_AT_6, CUT]);
    assert_eq!(notes([1.0; 3], true), [CUT]);
}

#[test]
fn why_a_volume_cannot_be_prepared_or_a_network_run_is_passed_on() {
    let thin = volume_of([20, 12, 8], axial([1.0; 3]), ramp);
    let (refused, stages) = run(&thin, Cut::Whole, left_at_low_x);
    assert_eq!(
        refused,
        Err(LabelError::Prepare(PrepareError::TooFewSlices))
    );
    assert_eq!(stages, [Stage::Preparing]);

    let volume = volume_of([20, 12, 16], axial([1.0; 3]), ramp);
    let mut networks = Stand {
        top: left_at_low_x,
        shut: Some(RunError::DeviceLost),
    };
    let failed = label(
        &volume,
        Cut::Whole,
        &lists(),
        ROBUST,
        &mut networks,
        &mut |_| (),
    );
    assert_eq!(failed, Err(LabelError::Run(RunError::DeviceLost)));
}

#[test]
fn every_refusal_is_a_plain_sentence() {
    let nothing = LabelError::NothingFound.to_string();
    assert_eq!(nothing, "The model found no brain in this scan.");
    // The other two say what they hold says, and nothing more.
    for inner in [PrepareError::TooFewSlices, PrepareError::TooLarge] {
        assert_eq!(LabelError::Prepare(inner).to_string(), inner.to_string());
    }
    for inner in [RunError::NoRoom, RunError::Failed("the log's".to_owned())] {
        assert_eq!(
            LabelError::Run(inner.clone()).to_string(),
            inner.to_string()
        );
    }
}

#[test]
fn the_robust_model_is_chosen_for_slices_more_than_three_millimetres_apart() {
    assert_eq!(THICK_MM, 3.0);
    assert!(choose([1.0, 1.0, 3.5], ModelChoice::Automatic));
    assert!(!choose([1.0, 1.0, 3.0], ModelChoice::Automatic));
    assert!(!choose([1.0; 3], ModelChoice::Automatic));
    // Whichever spacing is the long one.
    assert!(choose([3.5, 1.0, 1.0], ModelChoice::Automatic));
    assert!(choose([0.75, 6.0, 0.75], ModelChoice::Automatic));
    // A person's own choice stands whatever the spacing.
    for spacing in [[1.0; 3], [1.0, 1.0, 3.5], [8.0; 3]] {
        assert!(!choose(spacing, ModelChoice::Standard));
        assert!(choose(spacing, ModelChoice::Robust));
    }
}

/// Along x: the left structure up to 60, the right one up to 66, the left one again up to 70,
/// and nothing beyond. The scan of the test below begins 54 voxels in and is 20 long, so it
/// has 6 voxels of the left structure, 6 of the right, 4 more of the left and 4 of nothing.
fn left_in_two_pieces([x, _, _]: [usize; 3], _: [usize; 3]) -> u8 {
    match x {
        ..60 | 66..70 => 1,
        60..66 => 2,
        _ => 0,
    }
}

#[test]
fn a_robust_answer_is_tidied_the_fast_way_and_a_standard_one_the_standard_way() {
    let volume = volume_of([20, 12, 16], axial([1.0; 3]), ramp);
    // How many voxels the left structure and the right one have, by these networks.
    let found = |wanted: Wanted| {
        let mut networks = Stand {
            top: left_in_two_pieces,
            shut: None,
        };
        let labelled = label(
            &volume,
            Cut::Whole,
            &lists(),
            wanted,
            &mut networks,
            &mut |_| (),
        );
        let ids = labelled.expect("it is labelled").grid.ids;
        [LEFT, RIGHT].map(|label| ids.iter().filter(|&&id| id == label).count())
    };
    // Each voxel along x is a slab of 12 by 16. The fast way no structure loses a piece; the
    // standard way the left structure, a class of its own, loses its smaller one.
    assert_eq!(found(ROBUST), [10 * 192, 6 * 192]);
    assert_eq!(found(STANDARD), [6 * 192, 6 * 192]);
}
