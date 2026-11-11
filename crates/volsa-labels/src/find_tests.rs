//! Finding a part at a point and along a line of sight, pairing left with right.

use volsa_core::{Affine, Patient, Point};

use crate::find::{boxes_meet, first_part};
use crate::pairs::{difference_percent, pairs};
use crate::set::{LabelSet, Part};
use crate::table::{Side, Trust};

fn part(name: &str, group: &str, side: Side) -> Part {
    Part {
        name: name.to_owned(),
        group: group.to_owned(),
        side,
        colour: [200, 100, 50],
        trust: Trust::Model,
        voxels: 500,
        ml: 0.5,
    }
}

/// Ten voxels each way, 1 mm apart, with x and y running the other way round from the
/// patient's, as a NIfTI file's do. The first part fills the columns below 5, which is the
/// patient's left; the second fills the rest.
fn halves() -> LabelSet {
    let numbers = (0..1000_u16).map(|at| if at % 10 < 5 { 1 } else { 2 });
    LabelSet {
        dims: [10, 10, 10],
        numbers: numbers.collect(),
        placement: Affine::from_columns(
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [4.5, 4.5, 0.0],
        ),
        parts: vec![
            part("half", "Halves", Side::Left),
            part("half", "Halves", Side::Right),
        ],
        title: String::new(),
        verdict: None,
        made: None,
    }
}

/// The point of the patient at a place in the set, given in voxels.
fn at(set: &LabelSet, voxel: [f64; 3]) -> Point<Patient> {
    let [x, y, z] = voxel;
    set.placement.apply(Point::new(x, y, z))
}

#[test]
fn the_part_at_a_point_is_the_nearest_voxels() {
    let set = halves();
    assert_eq!(set.part_at(at(&set, [2.0, 3.0, 4.0])), Some(0));
    assert_eq!(set.part_at(at(&set, [7.0, 3.0, 4.0])), Some(1));
    assert_eq!(
        set.part_at(at(&set, [4.4, 3.2, 4.4])),
        Some(0),
        "nearest is voxel 4"
    );
    assert_eq!(
        set.part_at(at(&set, [4.6, 3.2, 4.4])),
        Some(1),
        "nearest is voxel 5"
    );
    assert_eq!(
        set.part_at(at(&set, [-0.4, 0.0, 0.0])),
        Some(0),
        "within the edge voxel"
    );
    for outside in [
        [-1.0, 3.0, 4.0],
        [10.0, 3.0, 4.0],
        [2.0, -0.6, 4.0],
        [2.0, 3.0, 9.6],
    ] {
        assert_eq!(set.part_at(at(&set, outside)), None, "{outside:?}");
    }
    assert_eq!(set.part_at(Point::new(f64::NAN, 0.0, 0.0)), None);
    let mut holed = halves();
    holed.numbers[0] = 0;
    assert_eq!(holed.part_at(at(&holed, [0.0; 3])), None, "0 is nothing");
    holed.numbers[1] = 9;
    assert_eq!(
        holed.part_at(at(&holed, [1.0, 0.0, 0.0])),
        None,
        "nor is a part not listed"
    );
}

#[test]
fn the_box_of_a_set_reaches_half_a_voxel_past_its_outermost_voxels() {
    let set = halves();
    let corners = set.corners();
    let reach = |axis: fn(&Point<Patient>) -> f64| {
        let along = corners.iter().map(axis);
        (
            along.clone().fold(f64::INFINITY, f64::min),
            along.fold(f64::NEG_INFINITY, f64::max),
        )
    };
    assert_eq!(reach(|corner| corner.x), (-5.0, 5.0));
    assert_eq!(reach(|corner| corner.y), (-5.0, 5.0));
    assert_eq!(reach(|corner| corner.z), (-0.5, 9.5));
    assert_eq!(set.finest_mm(), 1.0);
}

#[test]
fn a_line_of_sight_meets_the_first_part_that_is_shown() {
    let set = halves();
    // From the patient's right, looking towards their left.
    let from = Point::new(-20.0, 0.2, 4.3);
    let leftwards = [1.0, 0.0, 0.0];
    let (met, where_met) = first_part(&set, &[true, true], from, leftwards).expect("meets");
    assert_eq!(met, 1, "the right half is the nearer");
    assert!(
        (where_met.x + 5.0).abs() <= 1.0,
        "at the face it came in by: {where_met:?}"
    );
    assert!((where_met.y - 0.2).abs() < 1.0e-9 && (where_met.z - 4.3).abs() < 1.0e-9);
    let (met, where_met) = first_part(&set, &[true, false], from, leftwards).expect("meets");
    assert_eq!(met, 0, "with the right half hidden, the left behind it");
    assert!(
        (0.0..=1.0).contains(&where_met.x),
        "where the left half begins: {where_met:?}"
    );
    assert_eq!(first_part(&set, &[false, false], from, leftwards), None);
    assert_eq!(
        first_part(&set, &[], from, leftwards),
        None,
        "what is not listed is not shown"
    );
    // A direction need not be of length 1.
    let far = first_part(&set, &[true, true], from, [250.0, 0.0, 0.0]).expect("meets");
    assert_eq!(far.0, 1);
}

#[test]
fn a_line_that_misses_or_has_no_direction_meets_nothing() {
    let set = halves();
    let shown = [true, true];
    let beside = Point::new(-20.0, 30.0, 4.0);
    assert_eq!(first_part(&set, &shown, beside, [1.0, 0.0, 0.0]), None);
    let from = Point::new(-20.0, 0.0, 4.0);
    assert_eq!(
        first_part(&set, &shown, from, [-1.0, 0.0, 0.0]),
        None,
        "looking away"
    );
    assert_eq!(first_part(&set, &shown, from, [0.0; 3]), None);
    assert_eq!(first_part(&set, &shown, from, [f64::NAN, 0.0, 0.0]), None);
    // From inside, the part the eye is in is the first.
    let inside = Point::new(2.0, 0.0, 4.0);
    assert_eq!(
        first_part(&set, &shown, inside, [0.0, 0.0, 1.0]).map(|met| met.0),
        Some(0)
    );
}

#[test]
fn boxes_meet_when_their_reaches_overlap_every_way() {
    let set = halves();
    let moved = |by: f64| {
        set.corners()
            .map(|corner| Point::new(corner.x + by, corner.y, corner.z))
    };
    assert!(boxes_meet(&set.corners(), &set.corners()));
    assert!(
        boxes_meet(&set.corners(), &moved(9.0)),
        "a millimetre of overlap"
    );
    assert!(!boxes_meet(&set.corners(), &moved(100.0)));
    assert!(!boxes_meet(&set.corners(), &moved(-10.5)));
}

#[test]
fn a_name_with_two_sides_is_one_pair() {
    let parts = [
        part("hippocampus", "Deep brain", Side::Left),
        part("brain stem", "Cerebellum and brainstem", Side::Midline),
        part("hippocampus", "Deep brain", Side::Right),
        part("hippocampus", "Another group", Side::Left),
    ];
    let paired = pairs(&parts);
    let said: Vec<_> = paired
        .iter()
        .map(|pair| {
            (
                pair.name.as_str(),
                pair.group.as_str(),
                pair.left,
                pair.right,
                pair.midline,
            )
        })
        .collect();
    let expected = [
        ("hippocampus", "Deep brain", Some(0), Some(2), None),
        (
            "brain stem",
            "Cerebellum and brainstem",
            None,
            None,
            Some(1),
        ),
        ("hippocampus", "Another group", Some(3), None, None),
    ];
    assert_eq!(
        said, expected,
        "in the order first met; the same name in another group is another"
    );
    assert_eq!(pairs(&[]).len(), 0);
}

#[test]
fn the_difference_between_two_sides_is_a_share_of_their_mean() {
    assert_eq!(difference_percent(11.0, 9.0), Some(20.0));
    assert_eq!(difference_percent(9.0, 11.0), Some(-20.0));
    assert_eq!(difference_percent(5.0, 5.0), Some(0.0));
    assert_eq!(difference_percent(0.0, 0.0), None);
    assert_eq!(difference_percent(f64::NAN, 1.0), None);
}

#[test]
fn a_second_part_of_the_same_name_and_side_starts_a_pair_of_its_own_and_sides_fill_in_order() {
    let parts = [
        part("lobe", "Brain", Side::Left),
        part("lobe", "Brain", Side::Left),
        part("lobe", "Brain", Side::Right),
        part("lobe", "Brain", Side::Midline),
        part("lobe", "Brain", Side::Right),
        part("lobe", "Brain", Side::Right),
    ];
    let said: Vec<_> = pairs(&parts)
        .iter()
        .map(|pair| (pair.left, pair.right, pair.midline))
        .collect();
    assert_eq!(
        said,
        [
            (Some(0), Some(2), Some(3)),
            (Some(1), Some(4), None),
            (None, Some(5), None),
        ]
    );
}

#[test]
fn a_set_with_as_many_parts_as_a_set_may_have_is_paired_without_a_wait() {
    // A label file with no table names every number by itself: none pairs with another.
    let parts: Vec<_> = (0..65_535)
        .map(|number| part(&format!("Label {number}"), "", Side::Midline))
        .collect();
    let began = std::time::Instant::now();
    let paired = pairs(&parts);
    assert_eq!(paired.len(), parts.len());
    assert_eq!(paired.last().and_then(|pair| pair.midline), Some(65_534));
    // Going through every pair for every part took minutes; this is far inside a second.
    assert!(began.elapsed().as_secs() < 5, "{:?}", began.elapsed());
}
