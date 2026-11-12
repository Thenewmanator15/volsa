//! The made-up labels that `write_phantom` writes beside its made-up study read back as what
//! they were drawn as: a core, the two halves of a shell and a bead, on a grid of their own.

use std::path::Path;

use volsa_app::table_beside;
use volsa_core::{Patient, Point};
use volsa_labels::{LabelSet, Side, Trust, label_set, read_labels, read_table};

#[path = "../examples/write_phantom/labels.rs"]
mod labels;

/// The fine series of the phantom: its pixels and how far apart its slices are, in
/// millimetres.
const FINE: (f64, f64) = (0.6, 2.4);
/// The middle of the ball in that series.
const MIDDLE: [f64; 3] = [-0.3, -0.3, 46.8];

fn read_back() -> LabelSet {
    let grid = read_labels(&labels::volume(FINE.0, FINE.1)).expect("a label volume");
    let table = read_table("regions.json", labels::TABLE).expect("a table of names");
    label_set(&grid, Some(&table)).expect("few parts")
}

/// A point this far from the middle of the ball: towards the patient's left, and their back.
fn from_the_middle(left: f64, back: f64) -> Point<Patient> {
    let [x, y, z] = MIDDLE;
    Point::new(x + left, y + back, z)
}

#[test]
fn the_phantoms_labels_are_a_core_the_halves_of_a_shell_and_a_bead() {
    let set = read_back();
    let named: Vec<(&str, &str, Side)> = set
        .parts
        .iter()
        .map(|part| (part.name.as_str(), part.group.as_str(), part.side))
        .collect();
    let expected = [
        ("shell", "Outside", Side::Left),
        ("shell", "Outside", Side::Right),
        ("core", "Inside", Side::Midline),
        ("bead", "Outside", Side::Midline),
    ];
    assert_eq!(named, expected);
    assert_eq!(set.title, "Made-up phantom");
    // A ball 38.4 mm in radius with a core of half that, and a bead of 3.84 mm in the left
    // half of the shell: by the formula for a ball's volume, in millilitres.
    let about = |part: usize, ml: f64, within: f64| {
        let found = set.parts[part].ml;
        assert!((found - ml).abs() < within, "part {part}: {found} ml");
    };
    // The flat cut between the halves falls where the label voxels let it, up to half a
    // voxel off: over the shell's cross-section that is a millilitre or so either way.
    about(0, 103.53, 1.5);
    about(1, 103.77, 1.5);
    about(2, 29.65, 0.5);
    about(3, 0.237, 0.06);
    let whole: f64 = set.parts.iter().map(|part| part.ml).sum();
    assert!((whole - 237.18).abs() < 1.0, "the whole ball: {whole} ml");
}

#[test]
fn each_part_is_where_the_phantom_has_it_in_the_patient() {
    let set = read_back();
    assert_eq!(set.part_at(from_the_middle(0.0, 0.0)), Some(2), "the core");
    assert_eq!(set.part_at(from_the_middle(30.0, 0.0)), Some(0), "left");
    assert_eq!(set.part_at(from_the_middle(-30.0, 0.0)), Some(1), "right");
    let bead = from_the_middle(0.35 * 48.0, -0.55 * 48.0);
    assert_eq!(set.part_at(bead), Some(3), "towards the left and the front");
    assert_eq!(
        set.part_at(from_the_middle(47.0, 47.0)),
        None,
        "outside the ball"
    );
}

#[test]
fn the_labels_keep_a_grid_of_their_own_in_another_voxel_order() {
    let set = read_back();
    assert_eq!(set.dims, [64, 64, 64]);
    assert!(
        (set.finest_mm() - 1.5).abs() < 1e-6,
        "not the scan's 0.6 mm"
    );
    // Columns run towards the head, rows towards the front, slices towards the right: none
    // of them the way the scan's run.
    let ways = set.placement.columns();
    let expected = [[0.0, 0.0, 1.5], [0.0, -1.5, 0.0], [-1.5, 0.0, 0.0]];
    for (way, expected) in ways.iter().zip(expected) {
        let off = way.iter().zip(expected).map(|(a, b)| (a - b).abs());
        assert!(off.fold(0.0, f64::max) < 1e-6, "{ways:?}");
    }
}

#[test]
fn one_part_is_marked_approximate_and_the_made_up_check_fails() {
    let set = read_back();
    let trusted = |part: usize| set.parts[part].trust.clone();
    assert_eq!(trusted(0), Trust::Model);
    assert!(matches!(trusted(3), Trust::Approximate(how) if how.contains("rule")));
    let verdict = set.verdict.expect("the table carries a check");
    assert!(!verdict.passed);
    assert_eq!(verdict.low, [("shell".to_owned(), 0.52)]);
}

#[test]
fn the_table_is_found_beside_the_labels_by_its_name() {
    let labels = Path::new("phantom/labels/labels.nii.gz");
    let table = Path::new("phantom/labels/regions.json");
    let found = table_beside(labels, |path| path == table);
    assert_eq!(found.as_deref(), Some(table));
}
