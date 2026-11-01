//! Where measurement labels go when they would meet each other, the text in a corner, or the
//! edge of the picture.

use egui::{Pos2, Rect, pos2, vec2};

use super::{GAP, Placed, Wanted, place};

/// The picture the labels are written over: 400 by 300 points.
fn picture() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))
}

/// A label 60 by 12 points whose top left corner is at `(x, y)`, belonging to a point just
/// to the left of it and a little below.
fn label(x: f32, y: f32) -> Wanted {
    Wanted {
        rect: Rect::from_min_size(pos2(x, y), vec2(60.0, 12.0)),
        anchor: pos2(x - 6.0, y + 18.0),
    }
}

/// Whether no two of these labels meet.
fn apart(placed: &[Placed]) -> bool {
    placed.iter().enumerate().all(|(at, one)| {
        placed[at + 1..]
            .iter()
            .all(|other| !one.rect.intersects(other.rect))
    })
}

#[test]
fn labels_that_do_not_meet_stay_put() {
    let wanted = [label(20.0, 20.0), label(200.0, 100.0), label(50.0, 250.0)];
    let placed = place(&wanted, &[], picture());
    assert_eq!(placed.len(), wanted.len());
    for (want, got) in wanted.iter().zip(&placed) {
        assert_eq!(got.rect, want.rect);
        assert_eq!(got.leader, None, "nothing moved, so nothing to join");
    }
}

#[test]
fn crowded_labels_are_moved_apart_with_leaders() {
    // Four measurements ending at one point.
    let wanted = [label(100.0, 100.0); 4];
    let placed = place(&wanted, &[], picture());
    assert_eq!(placed.len(), 4);
    assert!(apart(&placed), "{placed:?}");
    // The first keeps its place; each of the others is further down, with a line to it.
    assert_eq!((placed[0].rect, placed[0].leader), (wanted[0].rect, None));
    for (above, label) in placed.iter().zip(&placed[1..]) {
        assert!(label.rect.top() > above.rect.bottom(), "{label:?}");
        let (from, to) = label.leader.expect("a moved label is joined to its shape");
        assert_eq!(from, wanted[0].anchor, "from where the shape is");
        assert_eq!(to.x, label.rect.left(), "to the label's left edge");
        assert!(label.rect.y_range().contains(to.y), "{to:?}");
    }
}

#[test]
fn a_label_stays_inside_its_picture() {
    let bounds = picture();
    // Partly off each side, and off the corner.
    let wanted = [
        label(380.0, 50.0),
        label(-20.0, 100.0),
        label(100.0, 295.0),
        label(100.0, -10.0),
        label(390.0, 299.0),
    ];
    let placed = place(&wanted, &[], bounds);
    for got in &placed {
        assert!(bounds.contains_rect(got.rect), "{got:?}");
    }
    assert!(apart(&placed));
    // Crowded at the bottom, where there is no room to move down: they move up instead.
    let crowded = place(&[label(100.0, 280.0); 3], &[], bounds);
    assert!(apart(&crowded), "{crowded:?}");
    for got in &crowded {
        assert!(bounds.contains_rect(got.rect), "{got:?}");
    }
    assert!(crowded[1].rect.top() < crowded[0].rect.top(), "{crowded:?}");
}

#[test]
fn a_label_keeps_clear_of_the_text_already_in_a_corner() {
    // Four lines of small type in the top left corner.
    let corner = Rect::from_min_size(pos2(10.0, 10.0), vec2(120.0, 56.0));
    let placed = place(&[label(20.0, 20.0)], &[corner], picture());
    assert!(!placed[0].rect.intersects(corner), "{placed:?}");
    assert!(placed[0].leader.is_some(), "it was moved aside");
    // One that was clear of it is left alone.
    let clear = place(&[label(200.0, 20.0)], &[corner], picture());
    assert_eq!(
        (clear[0].rect, clear[0].leader),
        (label(200.0, 20.0).rect, None)
    );
}

#[test]
fn with_no_clear_place_a_label_is_still_written_inside_the_picture() {
    // Room for two labels one above the other and no more, and three of them.
    let bounds = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 30.0));
    let placed = place(&[label(10.0, 0.0); 3], &[], bounds);
    assert_eq!(placed.len(), 3, "none is left out");
    for got in &placed {
        assert!(bounds.contains_rect(got.rect), "{got:?}");
    }
    assert!(!placed[0].rect.intersects(placed[1].rect));
}

#[test]
fn labels_in_one_row_keep_a_space_between_them_so_they_do_not_read_as_one() {
    // The second begins three points after the first ends: no overlap, but "Angle 2: 99.3"
    // and "Ruler 1: 38.4 mm" would run together as one line.
    let wanted = [label(100.0, 100.0), label(163.0, 100.0)];
    let placed = place(&wanted, &[], picture());
    assert_eq!(placed[0].rect, wanted[0].rect);
    let (first, second) = (placed[0].rect, placed[1].rect);
    let side_by_side = second.top() < first.bottom() && second.bottom() > first.top();
    let space = second.left() - first.right();
    assert!(!side_by_side || space >= 4.0, "{first:?} then {second:?}");
    assert!(placed[1].leader.is_some(), "it was moved: {placed:?}");
}

/// A label 60 by 12 points at `(100, 100)` whose shape is at `anchor`, with something already
/// written where it would stand, so that it must move.
fn moved_from_its_place(anchor: Pos2) -> Placed {
    let wanted = Wanted {
        rect: Rect::from_min_size(pos2(100.0, 100.0), vec2(60.0, 12.0)),
        anchor,
    };
    let blocker = wanted.rect;
    place(&[wanted], &[blocker], picture())[0]
}

#[test]
fn a_moved_region_label_never_covers_its_anchor_and_has_a_line() {
    // A region's label begins at its anchor's x, and one pulled left at the right edge begins
    // left of it: moved one step down it would stand on its own shape and have no line.
    let beginning_at = pos2(100.0, 112.0);
    let inside_its_width = pos2(130.0, 118.0);
    let near_its_end = pos2(155.0, 112.0);
    for anchor in [beginning_at, inside_its_width, near_its_end] {
        let placed = moved_from_its_place(anchor);
        assert!(
            !placed.rect.expand(GAP).contains(anchor),
            "{anchor:?} is under {placed:?}"
        );
        let (from, to) = placed.leader.expect("a moved label is joined to its shape");
        assert_eq!(from, anchor);
        assert!(
            from.distance(to) > 1.0,
            "a line of no length: {from:?} {to:?}"
        );
    }
}

#[test]
fn a_label_that_cannot_move_clear_of_its_anchor_stays_where_it_wanted() {
    // Room for two rows only, and the anchor in the second: the one place a step down would
    // stand on its own shape, so the label stays in its own, as crowded as that is.
    let bounds = Rect::from_min_size(pos2(0.0, 100.0), vec2(400.0, 26.0));
    let wanted = Wanted {
        rect: Rect::from_min_size(pos2(100.0, 100.0), vec2(60.0, 12.0)),
        anchor: pos2(130.0, 119.0),
    };
    let placed = place(&[wanted], &[wanted.rect], bounds);
    assert_eq!((placed[0].rect, placed[0].leader), (wanted.rect, None));
}
