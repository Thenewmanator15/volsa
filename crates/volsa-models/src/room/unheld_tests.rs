//! Tests of a discrete card's room when other programs hold its memory. The system's budget for
//! Volsa stays where it was as they take the card's memory, and what no program holds falls. A
//! run's needs, a quarter included, must fit in the budget, and what its networks take must fit
//! in what is free. Every card and figure here is made up.

use super::tests::{
    GIB, HEAD, MIB, STANDARD, WHOLE, card, id, on_card, on_processor, room, scored_on_processor,
};
use super::{Cut, Room, RunsOn, needs, rungs, why_not};
use crate::cards::CardKind;
use crate::network::Device;

/// What the networks of the whole scan take of a card, worked out here and not taken from the
/// module: 1 400 bytes a voxel, with no quarter added.
const TAKE: u64 = WHOLE * 1_400;
/// What the scores network takes of a card: 8.2 GiB, with no quarter added.
const SCORES: u64 = 82 * GIB / 10;

/// The first set-up's 16 GiB card with a budget of 14.5 GiB, and what no program holds of it.
fn filled_to(unheld: Option<u64>) -> Room {
    let mut room = room();
    room.budgets = vec![None, Some(145 * GIB / 10)];
    room.unheld = vec![None, unheld];
    room
}

#[test]
fn a_card_other_programs_have_filled_is_passed_over_for_the_processor() {
    // Other programs hold all but 2 GiB of the card, and the budget still says 14.5. The whole
    // scan's networks take 5.34 GiB of a card and the head's 2.61: neither fits, and the
    // processor has both.
    let full = filled_to(Some(2 * GIB));
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &full, RunsOn::Automatic),
        vec![on_processor(Cut::Whole), on_processor(Cut::Head)]
    );
    // With the card's memory free, or nothing known of it, the budget decides, as before: the
    // card comes first.
    let as_before = vec![
        on_card(1, Cut::Whole),
        on_processor(Cut::Whole),
        on_card(1, Cut::Head),
        on_processor(Cut::Head),
    ];
    for unheld in [Some(15 * GIB), None] {
        let room = filled_to(unheld);
        assert_eq!(
            rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Automatic),
            as_before,
            "{unheld:?}"
        );
    }
}

#[test]
fn the_budget_holds_what_a_run_needs_and_what_is_free_what_it_takes_to_the_byte() {
    let need = needs(WHOLE, STANDARD, Device::Card(id(1)));
    assert_eq!(need, TAKE + TAKE / 4);
    let cuts = |budget: Option<u64>, unheld: Option<u64>| {
        let mut room = room();
        room.budgets = vec![None, budget];
        room.unheld = vec![None, unheld];
        let found = rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(1)));
        found.iter().map(|rung| rung.cut).collect::<Vec<_>>()
    };
    let (both, head) = (vec![Cut::Whole, Cut::Head], vec![Cut::Head]);
    // What is free holds what the networks take, with no quarter added: a figure of what is
    // there now, and not a share to keep room within.
    assert_eq!(cuts(Some(100 * GIB), Some(TAKE)), both);
    assert_eq!(cuts(Some(100 * GIB), Some(TAKE - 1)), head);
    // The budget holds what they need, the quarter included, as before.
    assert_eq!(cuts(Some(need), Some(100 * GIB)), both);
    assert_eq!(cuts(Some(need - 1), Some(100 * GIB)), head);
    // Either alone decides where the other could not be read. With no budget, the card's own
    // memory less what Volsa holds stands in for it.
    assert_eq!(cuts(None, Some(TAKE)), both);
    assert_eq!(cuts(None, Some(TAKE - 1)), head);
    assert_eq!(cuts(Some(need - 1), None), head);
    // Nothing free leaves no room at all.
    assert_eq!(cuts(Some(100 * GIB), Some(0)), Vec::<Cut>::new());
}

#[test]
fn the_scores_network_stays_on_a_card_whose_free_memory_holds_what_it_takes() {
    // A card that other programs hold a few GiB of runs everything as before: the scores
    // network takes 8.2 GiB, and a quarter more is held to the budget only.
    let first = |unheld| {
        let found = rungs(WHOLE, HEAD, STANDARD, &filled_to(unheld), RunsOn::Automatic);
        found.first().copied()
    };
    assert_eq!(first(Some(SCORES)), Some(on_card(1, Cut::Whole)));
    assert_eq!(
        first(Some(SCORES - 1)),
        Some(scored_on_processor(1, Cut::Whole))
    );
}

#[test]
fn what_no_program_holds_already_counts_what_volsa_holds_for_drawing() {
    // The first set-up draws on card 1 and holds 1 GiB of it, and has no budget for it. That
    // hold is taken off the card's own memory, which stands in for the budget; what no program
    // holds has it taken off already, and is not cut twice.
    let cuts = |room: &Room| {
        let found = rungs(WHOLE, HEAD, STANDARD, room, RunsOn::Card(id(1)));
        found.iter().map(|rung| rung.cut).collect::<Vec<_>>()
    };
    let mut room = room();
    room.unheld = vec![None, Some(TAKE)];
    assert_eq!(cuts(&room), vec![Cut::Whole, Cut::Head]);
    // A missing entry is nothing read, and the hold is taken off as before.
    let need = needs(WHOLE, STANDARD, Device::Card(id(1)));
    room.unheld = Vec::new();
    room.cards[1].own_bytes = need + GIB - 1;
    assert_eq!(cuts(&room), vec![Cut::Head]);
}

#[test]
fn what_no_program_holds_lies_beside_its_card_and_leaves_shared_and_software_cards_alone() {
    let mut room = room();
    room.cards = vec![
        card(0, CardKind::Integrated, 128 * MIB),
        card(1, CardKind::Discrete, 16 * GIB),
        card(2, CardKind::Software, 32 * GIB),
        card(3, CardKind::Discrete, 16 * GIB),
    ];
    room.unheld = vec![Some(0), Some(2 * GIB), Some(32 * GIB), Some(5 * GIB)];
    // An integrated card shares the ordinary memory and has what is free of it; software has
    // no room; each discrete card has, in words, the lesser of its own two figures.
    room.budgets = vec![Some(GIB), Some(3 * GIB), Some(GIB), Some(4 * GIB)];
    let space = |card| room.space_on(Device::Card(id(card)));
    assert_eq!(space(0), Some(20 * GIB));
    assert_eq!(space(1), Some(2 * GIB));
    assert_eq!(space(2), None);
    assert_eq!(space(3), Some(4 * GIB));
}

#[test]
fn whether_a_run_fits_on_a_device_is_what_rungs_goes_by() {
    let full = filled_to(Some(TAKE - 1));
    let card = Device::Card(id(1));
    assert!(!full.has_room_for(card, WHOLE, STANDARD));
    assert!(full.has_room_for(card, HEAD, STANDARD));
    assert!(filled_to(Some(TAKE)).has_room_for(card, WHOLE, STANDARD));
    // The processor goes by the free ordinary memory; a card the computer does not list, or
    // software, has room for nothing.
    assert!(full.has_room_for(Device::Processor, WHOLE, STANDARD));
    assert!(!full.has_room_for(Device::Card(id(9)), 0, STANDARD));
}

#[test]
fn why_nothing_fits_speaks_of_what_is_free_on_the_card() {
    // A scan too large for the processor's 20 GiB whole or cut, on a card other programs have
    // all but filled: the card is said to have what no program holds, not its budget.
    let full = filled_to(Some(GIB / 2));
    let (whole, head) = (33_554_432, 30_000_000);
    assert_eq!(rungs(whole, head, STANDARD, &full, RunsOn::Automatic), []);
    let why = why_not(whole, head, STANDARD, &full);
    assert!(
        why.contains("The best graphics card has 0.5 GB to spare."),
        "{why}"
    );
}
