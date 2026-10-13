//! Tests of which devices a run fits on and in what order they are tried. Every volume, card
//! and figure here is made up.

use super::{Cut, Room, Rung, RunsOn, Wanted, needs, rungs};
use crate::cards::{Card, CardId, CardKind};
use crate::network::Device;

pub(super) const GIB: u64 = 1 << 30;
pub(super) const MIB: u64 = 1 << 20;
pub(super) const STANDARD: Wanted = Wanted {
    robust: false,
    cortex: false,
};
pub(super) const CORTEX: Wanted = Wanted {
    robust: false,
    cortex: true,
};
pub(super) const WHOLE: u64 = 4_096_000;
pub(super) const HEAD: u64 = 2_000_000;
/// The scores network's need, worked out here and not taken from the module: 8.2 GiB with a
/// quarter added.
const SCORES: u64 = 82 * GIB / 10;
const SCORES_NEED: u64 = SCORES + SCORES / 4;
/// The same for the processor, which it asks 3.1 GiB of, with a quarter added.
const SCORES_ON_THE_PROCESSOR: u64 = 31 * GIB / 10;
const SCORES_NEED_ON_THE_PROCESSOR: u64 = SCORES_ON_THE_PROCESSOR + SCORES_ON_THE_PROCESSOR / 4;

/// What the made-up card of a number is. It is never the number of a place in a list.
pub(super) fn id(card: u32) -> CardId {
    CardId(100 + u64::from(card))
}

/// The made-up card of a number, standing at the place of that number in the system's list.
pub(super) fn card(number: u32, kind: CardKind, own_bytes: u64) -> Card {
    Card {
        id: id(number),
        index: number,
        name: format!("Card {number}"),
        kind,
        own_bytes,
    }
}

/// An integrated card (card 0) of 128 MiB, a discrete card (card 1) of 16 GiB that Voxa
/// draws on and holds 1 GiB of, no budget read for either, and 20 GiB of ordinary memory free.
pub(super) fn room() -> Room {
    Room {
        cards: vec![
            card(0, CardKind::Integrated, 128 * MIB),
            card(1, CardKind::Discrete, 16 * GIB),
        ],
        budgets: vec![None, None],
        drawing_on: Some(id(1)),
        held: GIB,
        free_memory: 20 * GIB,
    }
}

/// A rung on a card, the scores network on the same card.
pub(super) fn on_card(card: u32, cut: Cut) -> Rung {
    Rung {
        device: Device::Card(id(card)),
        cut,
        scores_on: Device::Card(id(card)),
    }
}

/// A rung on the processor, the scores network there too.
pub(super) fn on_processor(cut: Cut) -> Rung {
    Rung {
        device: Device::Processor,
        cut,
        scores_on: Device::Processor,
    }
}

/// A rung on a card whose scores network goes to the processor.
pub(super) fn scored_on_processor(card: u32, cut: Cut) -> Rung {
    Rung {
        device: Device::Card(id(card)),
        cut,
        scores_on: Device::Processor,
    }
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
    // Too large whole for the card, and for the processor's 20 GiB.
    assert_eq!(
        rungs(33_554_432, 8_000_000, STANDARD, &room(), RunsOn::Automatic),
        vec![on_card(1, Cut::Head), on_processor(Cut::Head)]
    );
}

#[test]
fn with_little_ordinary_memory_free_the_head_is_offered_on_the_processor_alone() {
    // The first set-up (no budget known, so the card has 16 GiB less the 1 GiB Voxa holds),
    // with the ordinary memory down from 20 GiB to 10 GiB. The head needs 13.04 GiB on the
    // card, which has 15, and the scores network's 10.25 GiB fits there too, but a run on a
    // card asks the computer for 19.56 GiB of ordinary memory and one on the processor for
    // 8.38. The whole scan is too large for the card, and for the processor's 10 GiB.
    let mut room = room();
    room.free_memory = 10 * GIB;
    let found = rungs(16_777_216, 8_000_000, STANDARD, &room, RunsOn::Automatic);
    assert_eq!(found.len(), 1);
    let rung = found.first().copied().expect("one rung");
    assert_eq!(rung.device, Device::Processor);
    assert_eq!(rung.cut, Cut::Head);
    assert_eq!(rung.scores_on, Device::Processor);
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
    let need = needs(WHOLE, STANDARD, Device::Card(id(1)));
    assert_eq!(need, 7_168_000_000);
    let cuts = |own: u64, budgets: Vec<Option<u64>>, drawing_on: Option<u32>| {
        let room = Room {
            cards: vec![card(1, CardKind::Discrete, own)],
            budgets,
            drawing_on: drawing_on.map(id),
            held: GIB,
            free_memory: 20 * GIB,
        };
        let found = rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(1)));
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
    // With too little ordinary memory for it as well (it asks 3.875 GiB of the processor),
    // the scores network has nowhere to run, and no rung is offered, though the networks
    // before it would fit on the card. Less memory never offers more: 3 GiB and 2 GiB give
    // the same as each other.
    for free in [3 * GIB, 2 * GIB] {
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
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(1))),
        vec![on_card(1, Cut::Whole), on_card(1, Cut::Head)]
    );
    room.budgets = vec![None, Some(SCORES_NEED - 1)];
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(1))),
        vec![
            scored_on_processor(1, Cut::Whole),
            scored_on_processor(1, Cut::Head)
        ]
    );
    // Ordinary memory holds it to the byte too, where the processor is all there is: it asks
    // 3.1 GiB of the processor with a quarter added, whatever the scan. The scan here is an
    // empty one, so that nothing else a run asks for is more.
    room.free_memory = SCORES_NEED_ON_THE_PROCESSOR;
    assert_eq!(
        rungs(0, 0, STANDARD, &room, RunsOn::Processor),
        vec![on_processor(Cut::Whole), on_processor(Cut::Head)]
    );
    room.free_memory = SCORES_NEED_ON_THE_PROCESSOR - 1;
    assert_eq!(rungs(0, 0, STANDARD, &room, RunsOn::Processor), Vec::new());
}

#[test]
fn a_named_card_is_offered_whole_then_head_and_nothing_else() {
    let room = room();
    // An integrated card has the ordinary memory that is free, and holds a run there.
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(0))),
        vec![on_card(0, Cut::Whole), on_card(0, Cut::Head)]
    );
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(1))),
        vec![on_card(1, Cut::Whole), on_card(1, Cut::Head)]
    );
    // With little ordinary memory free, the integrated card has little too.
    let mut tight = room.clone();
    tight.free_memory = 4 * GIB;
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &tight, RunsOn::Card(id(0))),
        Vec::new()
    );
}

#[test]
fn an_integrated_cards_room_is_the_free_ordinary_memory_whatever_it_says_of_its_own() {
    // An integrated card shares the computer's memory: its room is what is free, and not
    // what it has of its own, nor a budget the system gives it. A software card has none, and
    // a discrete card has its budget.
    let mut room = room();
    room.cards = vec![
        card(0, CardKind::Integrated, 128 * MIB),
        card(1, CardKind::Discrete, 16 * GIB),
        card(2, CardKind::Software, 32 * GIB),
    ];
    room.budgets = vec![Some(GIB), Some(3 * GIB), Some(32 * GIB)];
    for free in [0, GIB, 20 * GIB, u64::MAX] {
        room.free_memory = free;
        assert_eq!(room.room_on(0, &room.cards[0]), Some(free));
    }
    assert_eq!(room.room_on(1, &room.cards[1]), Some(3 * GIB));
    assert_eq!(room.room_on(2, &room.cards[2]), None);
}

#[test]
fn a_card_that_is_not_there_or_is_software_is_never_offered() {
    let mut room = room();
    room.cards.push(card(2, CardKind::Software, 32 * GIB));
    room.budgets.push(Some(32 * GIB));
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(2))),
        Vec::new()
    );
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(9))),
        Vec::new()
    );
    // A card is asked for by what it is, not by its place in `cards`: here card 5 is second.
    room.cards = vec![
        card(0, CardKind::Integrated, 128 * MIB),
        card(5, CardKind::Discrete, 16 * GIB),
    ];
    room.budgets = vec![None, None];
    room.drawing_on = None;
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(5))),
        vec![on_card(5, Cut::Whole), on_card(5, Cut::Head)]
    );
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(1))),
        Vec::new()
    );
}

#[test]
fn a_card_is_the_same_card_wherever_the_systems_list_has_it() {
    // The two cards of the first set-up after the system has turned its list round: the
    // discrete card now stands at place 0 and the integrated one at place 1.
    let turned = |budget: Option<u64>| {
        let discrete = Card {
            index: 0,
            ..card(1, CardKind::Discrete, 16 * GIB)
        };
        let integrated = Card {
            index: 1,
            ..card(0, CardKind::Integrated, 128 * MIB)
        };
        let mut room = room();
        room.cards = vec![discrete, integrated];
        room.budgets = vec![budget, None];
        room
    };
    // Every rung is on the card it was on before, by what the card is.
    let before = rungs(WHOLE, HEAD, STANDARD, &room(), RunsOn::Automatic);
    assert_eq!(before.first(), Some(&on_card(1, Cut::Whole)));
    for runs_on in [RunsOn::Automatic, RunsOn::Card(id(1)), RunsOn::Card(id(0))] {
        let as_before = rungs(WHOLE, HEAD, STANDARD, &room(), runs_on);
        assert!(!as_before.is_empty(), "{runs_on:?}");
        let now = rungs(WHOLE, HEAD, STANDARD, &turned(None), runs_on);
        assert_eq!(now, as_before, "{runs_on:?}");
    }
    // No card is asked for by the place it stands at.
    for place in [0, 1] {
        let by_place = RunsOn::Card(CardId(place));
        let found = rungs(WHOLE, HEAD, STANDARD, &turned(None), by_place);
        assert_eq!(found, Vec::new());
    }
    // What Voxa holds is taken off the card it draws on, which is card 1 wherever it stands:
    // with a byte too little left for the whole scan, only the head is offered there.
    let need = needs(WHOLE, STANDARD, Device::Card(id(1)));
    let cuts = |room: &Room| {
        let found = rungs(WHOLE, HEAD, STANDARD, room, RunsOn::Card(id(1)));
        found.iter().map(|rung| rung.cut).collect::<Vec<_>>()
    };
    let mut tight = turned(None);
    tight.cards[0].own_bytes = need + GIB - 1;
    assert_eq!(cuts(&tight), vec![Cut::Head]);
    tight.drawing_on = Some(id(0));
    assert_eq!(cuts(&tight), vec![Cut::Whole, Cut::Head]);
    // A budget lies beside its card in the list as it was read.
    assert_eq!(cuts(&turned(Some(need - 1))), vec![Cut::Head]);
}

#[test]
fn the_processor_alone_is_offered_whole_then_head() {
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room(), RunsOn::Processor),
        vec![on_processor(Cut::Whole), on_processor(Cut::Head)]
    );
}
