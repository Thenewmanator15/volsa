//! Parts that are the same structure on the two sides of the patient, taken together, and
//! how much the two differ.

use std::collections::HashMap;

use crate::set::Part;
use crate::table::Side;

/// One name within a group, with the parts that carry it, by their place in the set's parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    /// The name the parts share.
    pub name: String,
    /// The group they are listed under.
    pub group: String,
    /// The part on the patient's left, if there is one.
    pub left: Option<usize>,
    /// The part on the patient's right.
    pub right: Option<usize>,
    /// The part on the midline, or with no side said.
    pub midline: Option<usize>,
}

/// The entries made so far for one name within a group, by their place among all the entries,
/// and how many of them have a part on the left, on the right and on the midline. A side's
/// parts fill the entries in order, so the first entry still free on a side is the one after
/// the last filled on it.
#[derive(Default)]
struct Named {
    entries: Vec<usize>,
    filled: [usize; 3],
}

/// The parts of a set as one entry for each name within a group, in the order first met. A
/// second part with the same name, group and side as one before starts an entry of its own.
/// The work grows with the number of parts and no faster, however many names there are.
pub fn pairs(parts: &[Part]) -> Vec<Pair> {
    let mut paired: Vec<Pair> = Vec::new();
    let mut named: HashMap<(&str, &str), Named> = HashMap::new();
    for (place, part) in parts.iter().enumerate() {
        let side = match part.side {
            Side::Left => 0,
            Side::Right => 1,
            Side::Midline => 2,
        };
        let known = named
            .entry((part.name.as_str(), part.group.as_str()))
            .or_default();
        let Some(filled) = known.filled.get_mut(side) else {
            continue;
        };
        let at = known.entries.get(*filled).copied().unwrap_or_else(|| {
            known.entries.push(paired.len());
            paired.push(Pair {
                name: part.name.clone(),
                group: part.group.clone(),
                left: None,
                right: None,
                midline: None,
            });
            paired.len() - 1
        });
        *filled += 1;
        if let Some(pair) = paired.get_mut(at) {
            match part.side {
                Side::Left => pair.left = Some(place),
                Side::Right => pair.right = Some(place),
                Side::Midline => pair.midline = Some(place),
            }
        }
    }
    paired
}

/// How much larger the left is than the right, as a percentage of the mean of the two; below
/// zero when the left is the smaller. `None` when both are nothing, or either is not a number.
pub fn difference_percent(left_ml: f64, right_ml: f64) -> Option<f64> {
    let mean = left_ml.midpoint(right_ml);
    let known = mean.is_finite() && mean > 0.0;
    known.then(|| (left_ml - right_ml) / mean * 100.0)
}
