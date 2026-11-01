//! Tests of the table that names what was labelled, and of the verdict in it. The lists are
//! made up, with a few of `SynthSeg`'s own labels.

use volsa_labels::{Entry, Side, Trust, Verdict, colour_for, verdict_in_words};
use volsa_models::{Lists, ModelSide, Structure, Wanted};

use crate::{FAIL_BELOW, table};

fn part(label: u32, name: &str, side: ModelSide) -> Structure {
    Structure {
        label,
        name: name.to_owned(),
        side,
        class: 0,
    }
}

/// Lists of three structures and three areas, each beginning with nothing, and score groups
/// of these names.
fn lists(groups: &[&str]) -> Lists {
    Lists {
        structures: vec![
            part(0, "background", ModelSide::Midline),
            part(17, "hippocampus", ModelSide::Left),
            part(16, "brain-stem", ModelSide::Midline),
        ],
        flip: vec![0, 1, 2],
        stage_classes: 2,
        areas: vec![
            part(0, "background", ModelSide::Midline),
            part(1035, "insula", ModelSide::Left),
            part(2035, "insula", ModelSide::Right),
        ],
        cortex: [3, 42],
        score_class: vec![0, 1, 2],
        score_groups: groups.iter().map(|&name| name.to_owned()).collect(),
        box_side: 32,
    }
}

const fn wanted(robust: bool, cortex: bool) -> Wanted {
    Wanted { robust, cortex }
}

/// The verdict on these scores, for groups called one, two and so on.
fn verdict(scores: &[f32]) -> Verdict {
    let names = ["one", "two", "three"];
    let table = table(&lists(&names[..scores.len()]), wanted(false, false), scores);
    table.verdict.expect("a table made here has a verdict")
}

#[test]
fn a_score_below_the_mark_fails_and_its_group_is_named() {
    assert_eq!(FAIL_BELOW, 0.65);
    let failed = Verdict {
        passed: false,
        fail_below: 0.65,
        low: vec![("two".to_owned(), 0.58)],
    };
    assert_eq!(verdict(&[0.8, 0.58]), failed);
    let said = verdict_in_words(&failed);
    assert!(said.contains("failed for two (0.58)"), "{said}");
    // Every group that failed is named, in the lists' order.
    let low = verdict(&[0.25, 0.9, 0.6431]).low;
    let both = [("one".to_owned(), 0.25), ("three".to_owned(), 0.6431)];
    assert_eq!(low, both);
}

#[test]
fn scores_on_the_mark_and_above_it_pass() {
    let passed = Verdict {
        passed: true,
        fail_below: 0.65,
        low: Vec::new(),
    };
    // The nearest single precision number to 0.65 is a little under it: it is 0.65 all the same.
    assert_eq!(verdict(&[0.65, 0.8, 1.0]), passed);
    let just_under = verdict(&[0.6499]);
    assert_eq!(just_under.low, [("one".to_owned(), 0.6499)]);
    assert!(!just_under.passed);
    // A score is read to its four decimal places: 0.64996 is 0.65 and passes, and 0.64994 is
    // 0.6499 and fails.
    assert_eq!(verdict(&[0.64996]), passed);
    assert_eq!(verdict(&[0.64994]).low, [("one".to_owned(), 0.6499)]);
    // Lists with no score group at all have nothing that can fail.
    assert_eq!(verdict(&[]), passed);
}

#[test]
fn a_score_that_is_no_number_fails() {
    let odd = verdict(&[0.9, f32::NAN]);
    assert!(!odd.passed);
    assert_eq!(odd.low.len(), 1);
    assert!(odd.low[0].0 == "two" && odd.low[0].1.is_nan());
}

#[test]
fn a_group_with_no_score_fails_and_a_score_with_no_group_is_passed_over() {
    let verdict_of = |groups: &[&str], scores: &[f32]| {
        let table = table(&lists(groups), wanted(false, false), scores);
        table.verdict.expect("a table made here has a verdict")
    };
    // Two groups and three scores: the third score is no group's, and is not read.
    let two = verdict_of(&["one", "two"], &[0.5, 0.9, 0.1]);
    assert_eq!(two.low, [("one".to_owned(), 0.5)]);
    // Three groups and one score, which passes. The two groups that were not scored fail,
    // and are named with no number for a score.
    let three = verdict_of(&["one", "two", "three"], &[0.9]);
    assert!(!three.passed);
    let named: Vec<&str> = three.low.iter().map(|(group, _)| group.as_str()).collect();
    assert_eq!(named, ["two", "three"]);
    assert!(three.low.iter().all(|(_, score)| score.is_nan()));
    // No scores at all: nothing was judged, so nothing passed.
    let none = verdict_of(&["one", "two"], &[]);
    assert!(!none.passed);
    let named: Vec<&str> = none.low.iter().map(|(group, _)| group.as_str()).collect();
    assert_eq!(named, ["one", "two"]);
    assert!(none.low.iter().all(|(_, score)| score.is_nan()));
    let said = verdict_in_words(&none);
    assert!(said.contains("failed for one (NaN), two (NaN)"), "{said}");
}

#[test]
fn the_title_says_which_model_and_whether_the_cortex_was_divided() {
    let title = |robust, cortex| table(&lists(&[]), wanted(robust, cortex), &[]).title;
    assert_eq!(title(false, false), "SynthSeg 2.0, standard model");
    assert_eq!(title(true, false), "SynthSeg 2.0, robust model");
    assert_eq!(
        title(false, true),
        "SynthSeg 2.0, standard model, with cortex areas"
    );
    assert_eq!(
        title(true, true),
        "SynthSeg 2.0, robust model, with cortex areas"
    );
}

#[test]
fn every_structure_and_area_is_named_with_its_group_its_side_and_its_colour() {
    // A name has one colour: the colour of the number that comes first for it.
    let entry = |name: &str, group: &str, side: Side, label: u32, first: u32| Entry {
        name: name.to_owned(),
        group: group.to_owned(),
        side,
        ids: vec![label],
        colour: colour_for(first),
        trust: Trust::Model,
    };
    // Nothing, the first of each list, is no part and has no name.
    let named = [
        entry("hippocampus", "Brain structures", Side::Left, 17, 17),
        entry("brain-stem", "Brain structures", Side::Midline, 16, 16),
        entry("insula", "Cortex areas", Side::Left, 1035, 1035),
        entry("insula", "Cortex areas", Side::Right, 2035, 1035),
    ];
    for cortex in [false, true] {
        let table = table(&lists(&["one"]), wanted(true, cortex), &[0.9]);
        assert_eq!(table.entries, named, "{cortex}");
    }
}
