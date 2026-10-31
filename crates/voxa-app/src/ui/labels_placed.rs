//! Where the words beside measurements are written, so that none is written over another, over
//! the text in the corners of a picture, or off the picture. Pure: it works on rectangles.

use egui::{Pos2, Rect, pos2, vec2};

/// The room left between one label and the next, and between a label and what is written in a
/// corner, above and below, in points.
const GAP: f32 = 2.0;
/// The room left to the side of a label, in points: more than above and below, so that two
/// labels in one row do not read as one run of words.
const SIDE: f32 = 8.0;

/// A label as it would be written if nothing were in the way.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wanted {
    /// Where its words would stand, beside the shape.
    pub rect: Rect,
    /// The point of the shape it belongs to, where a line to it begins.
    pub anchor: Pos2,
}

/// A label as it is written.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// Where its words stand.
    pub rect: Rect,
    /// When it was moved aside, the thin line to draw from the shape it belongs to (where it
    /// is anchored) to the label.
    pub leader: Option<(Pos2, Pos2)>,
}

/// Where each of the `wanted` labels is written, in the same order. A label keeps the place it
/// wanted, brought inside `bounds`, if that meets none of the places already `taken` and none
/// of the labels before it. If it does it moves down, a step at a time, each step its own
/// height and the gap between labels, as far as `bounds` reach, and then, if nowhere there was
/// clear, up; the moved label has a leader. Where nowhere is clear the label goes where it is
/// covered least, so none is ever left out.
pub fn place(wanted: &[Wanted], taken: &[Rect], bounds: Rect) -> Vec<Placed> {
    let mut placed: Vec<Placed> = Vec::with_capacity(wanted.len());
    for want in wanted {
        let home = within(want.rect, bounds);
        let blocked: Vec<Rect> = taken
            .iter()
            .copied()
            .chain(placed.iter().map(|label| label.rect))
            .collect();
        let covered = |slot: Rect| (overlap(slot, &blocked), slot);
        // The first of the slots to be least covered is the first that is clear, if one is.
        let least = slots(home, bounds).map(covered);
        let rect = least
            .min_by(|(one, _), (other, _)| one.total_cmp(other))
            .map_or(home, |(_, slot)| slot);
        let leader = (rect != home).then(|| (want.anchor, meeting(want.anchor, rect)));
        placed.push(Placed { rect, leader });
    }
    placed
}

/// `rect` moved the least that brings it inside `bounds`: left to right and top to bottom. One
/// too big for `bounds` stands at their left and top.
fn within(rect: Rect, bounds: Rect) -> Rect {
    let size = rect.size();
    let left = rect.left().min(bounds.right() - size.x).max(bounds.left());
    let top = rect.top().min(bounds.bottom() - size.y).max(bounds.top());
    Rect::from_min_size(pos2(left, top), size)
}

/// The places a label may stand in, best first: `home`, then each step down from it that stays
/// inside `bounds`, then each step up.
fn slots(home: Rect, bounds: Rect) -> impl Iterator<Item = Rect> {
    let step = home.height() + GAP;
    let walk = move |by: f32| {
        std::iter::successors(Some(home), move |slot| {
            let next = slot.translate(vec2(0.0, by));
            let inside = bounds.top() <= next.top() && next.bottom() <= bounds.bottom();
            inside.then_some(next)
        })
    };
    walk(step).chain(walk(-step).skip(1))
}

/// How much of the area of `slot`, with a margin of half `SIDE` to each side and half `GAP`
/// above and below, is covered by `blocked`: nothing when it is clear of them all.
fn overlap(slot: Rect, blocked: &[Rect]) -> f32 {
    let room = slot.expand2(vec2(SIDE / 2.0, GAP / 2.0));
    blocked
        .iter()
        .map(|other| {
            let shared = room.intersect(*other);
            shared.width().max(0.0) * shared.height().max(0.0)
        })
        .sum()
}

/// Where a leader from `anchor` meets the label at `rect`: the middle of its left edge when
/// the anchor lies to the left of it, as it does of a label beside its shape, which keeps the
/// line clear of the words of labels above it; otherwise the nearest point of the label.
fn meeting(anchor: Pos2, rect: Rect) -> Pos2 {
    if anchor.x < rect.left() {
        pos2(rect.left(), rect.center().y)
    } else {
        pos2(
            anchor.x.min(rect.right()).max(rect.left()),
            anchor.y.min(rect.bottom()).max(rect.top()),
        )
    }
}

#[cfg(test)]
mod tests;
