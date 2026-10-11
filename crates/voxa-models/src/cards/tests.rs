//! Tests of the list of graphics cards: what sort a card is, what it is known by, and where a
//! card stands in a list. The lists of the last are made up; the first test reads this
//! computer's own, whatever it holds.

use super::{Card, CardId, CardKind, budget, cards, identity, in_use, kind, place_of};

const GIB: u64 = 1 << 30;

/// A made-up list as a system might give it: each card at the place it is written.
fn listed(cards: &[(&str, u64, CardKind)]) -> Vec<Card> {
    let card = |(index, &(name, id, kind)): (u32, &(&str, u64, CardKind))| Card {
        id: CardId(id),
        index,
        name: name.to_owned(),
        kind,
        own_bytes: 0,
    };
    (0..).zip(cards).map(card).collect()
}

const GRAPHICS: (&str, u64, CardKind) = ("made-up graphics", 100, CardKind::Integrated);
const CARD: (&str, u64, CardKind) = ("a made-up card", 128, CardKind::Discrete);
const SOFTWARE: (&str, u64, CardKind) = ("drawn by the processor", 160, CardKind::Software);

#[test]
fn the_cards_are_listed_and_each_has_a_name_and_is_found_by_what_it_is() {
    let listed = cards();
    for (place, card) in listed.iter().enumerate() {
        assert!(!card.name.trim().is_empty(), "a card with no name");
        let mut earlier = listed.iter().take(place);
        assert!(
            earlier.all(|before| before.index < card.index),
            "out of order"
        );
        // A card is found where the first of its identity stands, which is its own place
        // unless the list holds it twice.
        let first = listed.iter().find(|other| other.id == card.id);
        assert_eq!(place_of(&listed, card.id), first.map(|first| first.index));
        // Asking what a card has to spare, or holds, never fails, whether or not the system
        // knows.
        let _ = (budget(card.id), in_use(card.id));
    }
    // No card is this one: nothing is known of it.
    let nowhere = CardId(u64::MAX);
    assert_eq!(place_of(&listed, nowhere), None);
    assert_eq!((budget(nowhere), in_use(nowhere)), (None, None));
}

#[test]
fn a_card_with_little_memory_of_its_own_is_integrated() {
    assert_eq!(kind(false, 16 * GIB), CardKind::Discrete);
    assert_eq!(kind(false, 2 * GIB), CardKind::Discrete);
    assert_eq!(kind(false, 2 * GIB - 1), CardKind::Integrated);
    assert_eq!(kind(false, 0), CardKind::Integrated);
    assert_eq!(kind(true, 16 * GIB), CardKind::Software);
    assert_eq!(kind(true, 0), CardKind::Software);
}

#[test]
fn a_card_is_found_by_what_it_is_wherever_the_list_now_has_it() {
    // The same three cards, before and after the system turned its list round.
    let before = listed(&[GRAPHICS, CARD, SOFTWARE]);
    let after = listed(&[CARD, GRAPHICS, SOFTWARE]);
    let (graphics, card) = (CardId(GRAPHICS.1), CardId(CARD.1));
    assert_eq!(place_of(&before, card), Some(1));
    assert_eq!(place_of(&after, card), Some(0));
    assert_eq!(place_of(&before, graphics), Some(0));
    assert_eq!(place_of(&after, graphics), Some(1));
    assert_eq!(place_of(&before, CardId(SOFTWARE.1)), Some(2));
    // The place is the list's own, not how far along the slice the card lies.
    let mut later = after.clone();
    later.remove(0);
    assert_eq!(place_of(&later, graphics), Some(1));
}

#[test]
fn a_card_the_list_holds_twice_is_found_at_its_first_place() {
    let twice = listed(&[GRAPHICS, CARD, GRAPHICS, SOFTWARE]);
    assert_eq!(place_of(&twice, CardId(GRAPHICS.1)), Some(0));
    assert_eq!(place_of(&twice, CardId(CARD.1)), Some(1));
    assert_eq!(place_of(&twice, CardId(SOFTWARE.1)), Some(3));
}

#[test]
fn a_card_that_has_gone_is_not_found() {
    let without = listed(&[GRAPHICS, SOFTWARE]);
    assert_eq!(place_of(&without, CardId(CARD.1)), None);
    assert_eq!(place_of(&[], CardId(CARD.1)), None);
    // A place is never taken for an identity: no card here is card 1, though one stands there.
    assert_eq!(place_of(&without, CardId(1)), None);
}

#[test]
fn a_cards_identity_is_both_halves_of_the_systems_number() {
    assert_eq!(identity(0, 0), CardId(0));
    assert_eq!(identity(0, 128), CardId(128));
    assert_eq!(identity(1, 0), CardId(1 << 32));
    assert_eq!(identity(1, 128), CardId((1 << 32) + 128));
    // Two cards that differ in either half are two cards, and a high half below zero is
    // carried as its bits.
    assert_ne!(identity(0, 128), identity(1, 128));
    assert_ne!(identity(0, 128), identity(0, 100));
    assert_eq!(identity(-1, u32::MAX), CardId(u64::MAX));
}
