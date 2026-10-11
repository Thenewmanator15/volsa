//! Tests of which devices a run fits on and in what order they are tried. Every volume, card
//! and figure here is made up.

use super::{Cut, Room, Rung, RunsOn, Wanted, needs, rungs, why_not};
use crate::cards::{Card, CardKind};
use crate::network::Device;

const GIB: u64 = 1 << 30;
const MIB: u64 = 1 << 20;
const STANDARD: Wanted = Wanted {
    robust: false,
    cortex: false,
};
const CORTEX: Wanted = Wanted {
    robust: false,
    cortex: true,
};
const WHOLE: u64 = 4_096_000;
const HEAD: u64 = 2_000_000;
/// The scores network's need, worked out here and not taken from the module: 8.2 GiB with a
/// quarter added.
const SCORES: u64 = 82 * GIB / 10;
const SCORES_NEED: u64 = SCORES + SCORES / 4;

fn card(index: u32, kind: CardKind, own_bytes: u64) -> Card {
    Card {
        index,
        name: format!("Card {index}"),
        kind,
        own_bytes,
    }
}

/// An integrated card (index 0) of 128 MiB, a discrete card (index 1) of 16 GiB that Voxa
/// draws on and holds 1 GiB of, no budget read for either, and 20 GiB of ordinary memory free.
fn room() -> Room {
    Room {
        cards: vec![
            card(0, CardKind::Integrated, 128 * MIB),
            card(1, CardKind::Discrete, 16 * GIB),
        ],
        budgets: vec![None, None],
        drawing_on: Some(1),
        held: GIB,
        free_memory: 20 * GIB,
    }
}

/// A rung on a card, the scores network on the same card.
fn on_card(index: u32, cut: Cut) -> Rung {
    Rung {
        device: Device::Card(index),
        cut,
        scores_on: Device::Card(index),
    }
}

/// A rung on the processor, the scores network there too.
fn on_processor(cut: Cut) -> Rung {
    Rung {
        device: Device::Processor,
        cut,
        scores_on: Device::Processor,
    }
}

/// A rung on a card whose scores network goes to the processor.
fn scored_on_processor(index: u32, cut: Cut) -> Rung {
    Rung {
        device: Device::Card(index),
        cut,
        scores_on: Device::Processor,
    }
}

#[test]
fn a_run_needs_its_largest_network_per_voxel_with_a_quarter_added() {
    assert_eq!(
        needs(WHOLE, STANDARD, Device::Card(1)),
        4_096_000 * 1_400 * 5 / 4
    );
    assert_eq!(
        needs(WHOLE, STANDARD, Device::Processor),
        4_096_000 * 2_000 * 5 / 4
    );
    // The cortex network is the largest on the processor and no larger than the rest on a card.
    assert_eq!(
        needs(WHOLE, CORTEX, Device::Processor),
        4_096_000 * 2_300 * 5 / 4
    );
    assert_eq!(
        needs(WHOLE, CORTEX, Device::Card(1)),
        needs(WHOLE, STANDARD, Device::Card(1))
    );
    // The robust networks need no more than the standard one.
    let robust = Wanted {
        robust: true,
        cortex: false,
    };
    assert_eq!(
        needs(WHOLE, robust, Device::Processor),
        needs(WHOLE, STANDARD, Device::Processor)
    );
    // The card a run goes to makes no difference to its need, only the kind of device.
    assert_eq!(
        needs(WHOLE, STANDARD, Device::Card(0)),
        needs(WHOLE, STANDARD, Device::Card(7))
    );
    assert_eq!(needs(0, STANDARD, Device::Processor), 0);
    // A count too large to multiply is as much as can be counted, not a wrong small figure.
    assert_eq!(needs(u64::MAX, STANDARD, Device::Processor), u64::MAX);
}

#[test]
fn a_scan_that_fits_everywhere_is_tried_whole_then_head_each_card_before_the_processor() {
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room(), RunsOn::Automatic),
        vec![
            on_card(1, Cut::Whole),
            on_processor(Cut::Whole),
            on_card(1, Cut::Head),
            on_processor(Cut::Head),
        ]
    );
}

#[test]
fn a_scan_too_large_whole_is_offered_cut_to_the_head() {
    assert_eq!(
        rungs(16_777_216, 8_000_000, STANDARD, &room(), RunsOn::Automatic),
        vec![on_card(1, Cut::Head), on_processor(Cut::Head)]
    );
}

#[test]
fn automatic_offers_only_discrete_cards_in_the_systems_order() {
    let mut room = room();
    // The integrated card is first, a software card sits among the discrete ones, and the
    // second discrete card has a budget that holds the head but not the whole scan.
    room.cards = vec![
        card(0, CardKind::Integrated, 128 * MIB),
        card(1, CardKind::Discrete, 16 * GIB),
        card(2, CardKind::Software, 32 * GIB),
        card(3, CardKind::Discrete, 24 * GIB),
    ];
    room.budgets = vec![None, None, None, Some(5 * GIB)];
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Automatic),
        vec![
            on_card(1, Cut::Whole),
            on_processor(Cut::Whole),
            on_card(1, Cut::Head),
            scored_on_processor(3, Cut::Head),
            on_processor(Cut::Head),
        ]
    );
    // With no card at all the processor is all there is.
    room.cards.clear();
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Automatic),
        vec![on_processor(Cut::Whole), on_processor(Cut::Head)]
    );
}

#[test]
fn a_discrete_cards_room_is_its_budget_else_its_memory_less_what_voxa_holds() {
    let need = needs(WHOLE, STANDARD, Device::Card(1));
    assert_eq!(need, 7_168_000_000);
    let cuts = |own: u64, budgets: Vec<Option<u64>>, drawing_on: Option<u32>| {
        let room = Room {
            cards: vec![card(1, CardKind::Discrete, own)],
            budgets,
            drawing_on,
            held: GIB,
            free_memory: 20 * GIB,
        };
        let found = rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(1));
        found.iter().map(|rung| rung.cut).collect::<Vec<_>>()
    };
    let both = vec![Cut::Whole, Cut::Head];
    let head = vec![Cut::Head];
    // The budget is the room, whatever the card's own memory is: here it is more than the
    // card has and, in the next, less.
    assert_eq!(cuts(GIB, vec![Some(need)], Some(1)), both);
    assert_eq!(cuts(100 * GIB, vec![Some(need - 1)], Some(1)), head);
    // With no budget, Voxa's own hold is taken off the card it draws on, to the byte.
    assert_eq!(cuts(need + GIB, vec![None], Some(1)), both);
    assert_eq!(cuts(need + GIB - 1, vec![None], Some(1)), head);
    // A missing entry is no budget.
    assert_eq!(cuts(need + GIB, vec![], Some(1)), both);
    assert_eq!(cuts(need + GIB - 1, vec![], Some(1)), head);
    // The hold is not taken off a card Voxa does not draw on, or when it draws on none.
    assert_eq!(cuts(need + GIB - 1, vec![None], Some(0)), both);
    assert_eq!(cuts(need + GIB - 1, vec![None], None), both);
    // A hold larger than the card leaves it no room, and does not wrap round.
    assert_eq!(cuts(GIB / 2, vec![None], Some(1)), Vec::<Cut>::new());
}

#[test]
fn the_scores_network_goes_to_the_processor_when_the_card_is_too_small_for_it() {
    let mut room = room();
    room.budgets = vec![None, Some(8 * GIB)];
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Automatic),
        vec![
            scored_on_processor(1, Cut::Whole),
            on_processor(Cut::Whole),
            scored_on_processor(1, Cut::Head),
            on_processor(Cut::Head),
        ]
    );
    // With too little ordinary memory for it as well, the scores network has nowhere to run,
    // and no rung is offered, though the networks before it would fit on the card. Less
    // memory never offers more: 6 GiB and 4 GiB give the same as each other.
    for free in [6 * GIB, 4 * GIB] {
        room.free_memory = free;
        assert_eq!(
            rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Automatic),
            Vec::new()
        );
    }
}

#[test]
fn the_scores_network_stays_on_a_device_that_just_holds_it() {
    let mut room = room();
    room.budgets = vec![None, Some(SCORES_NEED)];
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(1)),
        vec![on_card(1, Cut::Whole), on_card(1, Cut::Head)]
    );
    room.budgets = vec![None, Some(SCORES_NEED - 1)];
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(1)),
        vec![
            scored_on_processor(1, Cut::Whole),
            scored_on_processor(1, Cut::Head)
        ]
    );
    // Ordinary memory holds it to the byte too, where the processor is all there is.
    room.free_memory = SCORES_NEED;
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Processor),
        vec![on_processor(Cut::Whole), on_processor(Cut::Head)]
    );
    room.free_memory = SCORES_NEED - 1;
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Processor),
        Vec::new()
    );
}

#[test]
fn a_named_card_is_offered_whole_then_head_and_nothing_else() {
    let room = room();
    // An integrated card has the ordinary memory that is free, and holds a run there.
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(0)),
        vec![on_card(0, Cut::Whole), on_card(0, Cut::Head)]
    );
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(1)),
        vec![on_card(1, Cut::Whole), on_card(1, Cut::Head)]
    );
    // With little ordinary memory free, the integrated card has little too.
    let mut tight = room.clone();
    tight.free_memory = 6 * GIB;
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &tight, RunsOn::Card(0)),
        Vec::new()
    );
}

#[test]
fn a_card_that_is_not_there_or_is_software_is_never_offered() {
    let mut room = room();
    room.cards.push(card(2, CardKind::Software, 32 * GIB));
    room.budgets.push(Some(32 * GIB));
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(2)),
        Vec::new()
    );
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(9)),
        Vec::new()
    );
    // The number is the system's own, not a place in the list: here card 5 is second.
    room.cards = vec![
        card(0, CardKind::Integrated, 128 * MIB),
        card(5, CardKind::Discrete, 16 * GIB),
    ];
    room.budgets = vec![None, None];
    room.drawing_on = None;
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(5)),
        vec![on_card(5, Cut::Whole), on_card(5, Cut::Head)]
    );
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(1)),
        Vec::new()
    );
}

#[test]
fn the_processor_alone_is_offered_whole_then_head() {
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room(), RunsOn::Processor),
        vec![on_processor(Cut::Whole), on_processor(Cut::Head)]
    );
}

#[test]
fn the_cortex_network_asks_more_of_the_processor() {
    let mut room = room();
    room.free_memory = 11_500_000_000;
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Processor),
        vec![on_processor(Cut::Whole), on_processor(Cut::Head)]
    );
    assert_eq!(
        rungs(WHOLE, HEAD, CORTEX, &room, RunsOn::Processor),
        vec![on_processor(Cut::Head)]
    );
}

#[test]
fn when_nothing_fits_there_are_no_rungs_and_why_not_says_what_is_missing() {
    let (whole, head) = (16_777_216, 8_000_000);
    let mut room = room();
    room.budgets = vec![None, Some(5 * GIB)];
    room.free_memory = 4 * GIB;
    assert_eq!(
        rungs(whole, head, STANDARD, &room, RunsOn::Automatic),
        Vec::new()
    );
    // Needs are rounded up and what there is is rounded down, so the figures never seem to
    // fit when they do not: the head needs 13.04 GB on a card and 18.63 on the processor, and
    // the scores network 10.25.
    assert_eq!(
        why_not(whole, head, STANDARD, &room),
        concat!(
            "There is not enough memory to label this scan. ",
            "At its smallest it needs about 13.1 GB on a graphics card or 18.7 GB on the ",
            "processor, and scoring the result needs about 10.3 GB on either. ",
            "The best graphics card has 5.0 GB to spare. ",
            "The computer has 4.0 GB of ordinary memory free."
        )
    );
}

#[test]
fn why_not_names_the_roomiest_discrete_card_or_says_there_is_none() {
    let mut room = room();
    room.cards = vec![
        card(0, CardKind::Integrated, 128 * MIB),
        card(1, CardKind::Discrete, 6 * GIB),
        card(2, CardKind::Discrete, 9 * GIB),
        card(3, CardKind::Software, 64 * GIB),
    ];
    room.budgets = vec![];
    room.drawing_on = Some(2);
    room.held = GIB / 2;
    room.free_memory = GIB + GIB / 4;
    let said = why_not(16_777_216, 16_777_216, CORTEX, &room);
    // Card 2 has 9 GiB less the half held there, which is 8.5 GB, and the software card is
    // not counted; the cortex network is the larger need on the processor.
    assert!(said.contains("The best graphics card has 8.5 GB to spare."));
    assert!(said.contains("The computer has 1.2 GB of ordinary memory free."));
    assert!(said.contains("about 27.4 GB on a graphics card or 45.0 GB on the processor"));

    room.cards = vec![card(0, CardKind::Integrated, 128 * MIB)];
    let said = why_not(16_777_216, 16_777_216, STANDARD, &room);
    assert!(said.contains("There is no graphics card with memory of its own."));
    assert!(!said.contains("to spare"));
}
