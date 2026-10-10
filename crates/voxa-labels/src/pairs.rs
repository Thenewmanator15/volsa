//! Parts that are the same structure on the two sides of the patient, taken together, and
//! how much the two differ.

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

/// The parts of a set as one entry for each name within a group, in the order first met. A
/// second part with the same name, group and side as one before starts an entry of its own.
pub fn pairs(parts: &[Part]) -> Vec<Pair> {
    let mut paired: Vec<Pair> = Vec::new();
    for (place, part) in parts.iter().enumerate() {
        let fits = |pair: &&mut Pair| {
            let free = match part.side {
                Side::Left => pair.left.is_none(),
                Side::Right => pair.right.is_none(),
                Side::Midline => pair.midline.is_none(),
            };
            pair.name == part.name && pair.group == part.group && free
        };
        let mut fresh = Pair {
            name: part.name.clone(),
            group: part.group.clone(),
            left: None,
            right: None,
            midline: None,
        };
        let pair = match paired.iter_mut().find(fits) {
            Some(pair) => pair,
            None => &mut fresh,
        };
        match part.side {
            Side::Left => pair.left = Some(place),
            Side::Right => pair.right = Some(place),
            Side::Midline => pair.midline = Some(place),
        }
        if fresh.left.or(fresh.right).or(fresh.midline).is_some() {
            paired.push(fresh);
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
