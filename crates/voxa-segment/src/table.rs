//! The table that names what a run labelled, with the model's verdict on its own work.
//!
//! The names and the sides are those of the model folder's lists. Every structure and every
//! area of the cortex is named, whether or not a voxel took its label: a name whose number
//! is not among the labels makes no part of a label set. Nothing, label 0, is the first of
//! each list and is not named.
//!
//! The verdict is made from `SynthSeg`'s own scores, one for each score group of the lists. A
//! group fails when its score is below 0.65, and when it has no score: nothing is said to
//! have passed that was not scored.

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
/// Each name is listed under "Brain structures" or "Cortex areas", in the order of the
/// lists, with the side the lists give it, the colour Voxa gives its number, and the trust
/// due to a model.
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
        entries,
        verdict: Some(verdict(&lists.score_groups, scores)),
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

/// A name from the lists, no longer than a table keeps one.
fn kept(name: &str) -> String {
    name.chars().take(MAX_NAME).collect()
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
