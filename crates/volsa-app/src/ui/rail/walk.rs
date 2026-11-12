//! Walking the rail with the keyboard. With a card, or the button of its menu, in focus, the
//! up and down arrows move the focus to the card before and the card after, passing over the
//! headings between them and scrolling to a card that is out of sight; Enter opens a card, as
//! a click does. Tab moves through the cards and their buttons one after the other, as it does
//! through any other buttons.
//!
//! The arrows are kept for the rail while a card is in focus, so the keys that step through
//! the slices are not also pressed. Only the rows in sight are drawn, so a card out of sight is
//! found among the rows and brought into sight first.

use egui::{
    EventFilter, Id, Key, Modifiers, Pos2, Rect, Response, Ui, style::ScrollAnimation, vec2,
};
use volsa_dicom::Index;

use super::kept::Laid;
use super::rows::Row;
use super::{card, document_card};

/// The keys a card in focus keeps for itself: egui must not also move the focus by them.
const KEPT: EventFilter = EventFilter {
    tab: false,
    horizontal_arrows: false,
    vertical_arrows: true,
    escape: false,
};

/// Whether the card drawn with `card` as its response, or the button of its menu `dots`, has
/// the keyboard. While it has, the up and down arrows are the rail's.
pub fn holds_keys(ui: &Ui, card: &Response, dots: &Response) -> bool {
    let held = card.has_focus() || dots.has_focus();
    if held {
        ui.memory_mut(|memory| {
            for id in [card.id, dots.id] {
                memory.set_focus_lock_filter(id, KEPT);
            }
        });
    }
    held
}

/// Where the rail's rows are drawn: the corner of the first, and how wide each is.
#[derive(Debug, Clone, Copy)]
pub struct Where {
    /// The top left corner of the list.
    pub origin: Pos2,
    /// How wide each row is.
    pub wide: f32,
}

/// The identifier of the card in `row`, or `None` for a heading.
fn card_in(row: Row, index: &Index) -> Option<Id> {
    match row {
        Row::Card { study, series } => {
            let listed = index.studies.get(study)?.series.get(series)?;
            Some(card::id_of(&listed.id))
        }
        Row::Document(place) => Some(document_card::id_of(index.documents.get(place)?)),
        Row::Person(_) | Row::Study(_) | Row::Unplaced | Row::Other(_) => None,
    }
}

/// The row of the card next to the one in row `from`, in this direction, passing over headings.
fn next_card(rows: &[Row], from: usize, forwards: bool) -> Option<usize> {
    let is_card =
        |place: &usize| matches!(rows.get(*place), Some(Row::Card { .. } | Row::Document(_)));
    if forwards {
        (from + 1..rows.len()).find(is_card)
    } else {
        (0..from).rev().find(is_card)
    }
}

/// Act on the up and down arrows pressed this frame, with the card in row `from` in focus:
/// the focus goes to the card that many cards on, which is scrolled into sight. The keys are
/// taken, so that nothing else acts on them.
pub fn walk(ui: &Ui, from: usize, laid: &Laid, index: &Index, at: Where) {
    let (down, up) = ui.input_mut(|input| {
        let down = input.count_and_consume_key(Modifiers::NONE, Key::ArrowDown);
        (
            down,
            input.count_and_consume_key(Modifiers::NONE, Key::ArrowUp),
        )
    });
    let mut place = from;
    for (times, forwards) in [(down, true), (up, false)] {
        for _ in 0..times {
            place = next_card(&laid.rows, place, forwards).unwrap_or(place);
        }
    }
    if place == from {
        return;
    }
    let target = laid
        .rows
        .get(place)
        .copied()
        .and_then(|row| card_in(row, index));
    let top = laid.tops.get(place).copied();
    if let (Some(id), Some(top)) = (target, top) {
        let size = vec2(at.wide, card::HEIGHT);
        let rect = Rect::from_min_size(at.origin + vec2(0.0, top), size);
        // At once, not eased in: a card that is not drawn in the frame after the focus goes to
        // it loses the focus.
        ui.scroll_to_rect_animation(rect, None, ScrollAnimation::none());
        ui.memory_mut(|memory| memory.request_focus(id));
    }
}
