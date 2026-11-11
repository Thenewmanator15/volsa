//! The table that names what a run labelled, with the model's verdict on its own work.
//!
//! The names and the sides are those of the model folder's lists. Every structure and every
//! area of the cortex is named, whether or not a voxel took its label: a name whose number
//! is not among the labels makes no part of a label set. Nothing, label 0, is the first of
//! each list and is not named.
//!
//! The table is made in the form a reader gives back: each name once, its sides together, left
//! before right before midline, the names in the order they first come, and one colour for a
//! name. So a table written to a file and read again is the same table.
//!
//! The verdict is made from `SynthSeg`'s own scores, one for each score group of the lists. A
//! group fails when its score is below 0.65, and when it has no score: nothing is said to
//! have passed that was not scored.

use std::collections::HashMap;

use voxa_labels::{Entry, MAX_NAME, Side, Table, Trust, Verdict, colour_for};
use voxa_models::{Lists, ModelSide, Structure, Wanted};

/// A score group fails when its score is below this.
pub const FAIL_BELOW: f64 = 0.65;
/// The group the structures are listed under.
const STRUCTURES: &str = "Brain structures";
/// The group the areas of the cortex are listed under.
const AREAS: &str = "Cortex areas";
/// A score has four decimal places: so many parts of one.
const PARTS: f64 = 10_000.0;

/// The table that goes with the labels of a run: a name for every structure and every area
/// of the cortex in `lists`, a title that says which model made the labels, and the model's
/// verdict on its own work.
///
/// Each name is listed under "Brain structures" or "Cortex areas", with the side the lists
/// give it and the trust due to a model. A name that has two sides is listed once for each,
/// together, left before right: names come in the order the lists first give them. A name
/// has one colour, the one Voxa gives the number that comes first for it in the table.
///
/// `scores` are what the scores network gave: one for each score group of the lists, in
/// their order. The verdict fails when any is below [`FAIL_BELOW`], and names each group
/// that is, with its score, in the lists' order. A score that is not a number fails too,
/// and so does a group for which there is no score: it is named with no number for its
/// score. A score beyond the last group is not read.
pub fn table(lists: &Lists, wanted: Wanted, scores: &[f32]) -> Table {
    let named = |parts: &[Structure], group: &str| -> Vec<Entry> {
        let parts = parts.iter().filter(|part| part.label != 0);
        parts.map(|part| entry(part, group)).collect()
    };
    let mut entries = named(&lists.structures, STRUCTURES);
    entries.extend(named(&lists.areas, AREAS));
    let model = if wanted.robust { "robust" } else { "standard" };
    let cortex = if wanted.cortex {
        ", with cortex areas"
    } else {
        ""
    };
    Table {
        title: format!("SynthSeg 2.0, {model} model{cortex}"),
        entries: as_a_reader_gives_them(entries),
        verdict: Some(verdict(&lists.score_groups, scores)),
        made: None,
    }
}

/// The name of one structure or area, listed under `group`.
fn entry(part: &Structure, group: &str) -> Entry {
    Entry {
        name: kept(&part.name),
        group: group.to_owned(),
        side: match part.side {
            ModelSide::Left => Side::Left,
            ModelSide::Right => Side::Right,
            ModelSide::Midline => Side::Midline,
        },
        ids: vec![part.label],
        colour: colour_for(part.label),
        trust: Trust::Model,
    }
}

/// The entries gathered by name, in the order the names first come, each name's sides left
/// before right before midline, and each name in one colour: the colour of the number that
/// comes first. This is the form in which a table read from a file gives them.
fn as_a_reader_gives_them(entries: Vec<Entry>) -> Vec<Entry> {
    let mut names: Vec<Vec<Entry>> = Vec::new();
    let mut place: HashMap<String, usize> = HashMap::new();
    for entry in entries {
        if let Some(sides) = place.get(&entry.name).and_then(|at| names.get_mut(*at)) {
            sides.push(entry);
        } else {
            place.insert(entry.name.clone(), names.len());
            names.push(vec![entry]);
        }
    }
    names.into_iter().flat_map(in_one_colour).collect()
}

/// The sides of one name, left before right before midline, each in the colour of the first.
fn in_one_colour(mut sides: Vec<Entry>) -> Vec<Entry> {
    sides.sort_by_key(|entry| match entry.side {
        Side::Left => 0,
        Side::Right => 1,
        Side::Midline => 2,
    });
    let first = sides.first().and_then(|entry| entry.ids.first());
    if let Some(colour) = first.map(|label| colour_for(*label)) {
        for entry in &mut sides {
            entry.colour = colour;
        }
    }
    sides
}

/// A name from the lists as a table keeps it: with no space at either end, no longer than
/// a table keeps one, and again with no space at the end the cut left.
fn kept(name: &str) -> String {
    let cut: String = name.trim().chars().take(MAX_NAME).collect();
    cut.trim_end().to_owned()
}

/// The model's verdict on its own work: failed when a group scored below the mark, was
/// given a score that is no number, or was given none; with every group that did.
fn verdict(groups: &[String], scores: &[f32]) -> Verdict {
    // A group with no score is judged as one whose score is no number.
    let given = scores.iter().copied().chain(std::iter::repeat(f32::NAN));
    let each = groups.iter().zip(given);
    let each = each.map(|(group, score)| (kept(group), four_places(score)));
    let low: Vec<(String, f64)> = each
        .filter(|(_, score)| score.is_nan() || *score < FAIL_BELOW)
        .collect();
    Verdict {
        passed: low.is_empty(),
        fail_below: FAIL_BELOW,
        low,
    }
}

/// A score as the number its four decimal places write. The scores come in single precision,
/// in which 0.65 is a little under 0.65: read as it stands it would fail a mark it meets.
fn four_places(score: f32) -> f64 {
    (f64::from(score) * PARTS).round() / PARTS
}
