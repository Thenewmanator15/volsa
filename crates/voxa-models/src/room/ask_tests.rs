//! Tests of what a run asks of ordinary memory, of how that holds a rung back, and of the words
//! said when nothing fits. Every volume, card and figure here is made up. The sums are worked
//! out here from the module's figures, a GiB being 1 073 741 824 bytes, and not taken from it.

use super::tests::{
    CORTEX, GIB, HEAD, MIB, STANDARD, WHOLE, card, id, on_card, on_processor, room,
    scored_on_processor,
};
use super::{Cut, Rung, RunsOn, Wanted, asks, rungs, why_not};
use crate::cards::CardKind;
use crate::network::Device;

#[test]
fn a_run_asks_for_voxas_own_and_the_larger_of_its_networks_and_its_scores_with_a_quarter() {
    // 4 096 000 voxels. Voxa's own is 200 bytes each: 819 200 000. The scores network asks
    // for 8.4 GiB where it runs on a card, 9 019 431 321, and 3.1 GiB on the processor,
    // 3 328 599 654.
    //
    // The networks on a card ask for 1 900 bytes a voxel, 7 782 400 000, which is less than
    // the scores on a card: 819 200 000 + 9 019 431 321 = 9 838 631 321, and a quarter more
    // is 12 298 289 151. The cortex network makes no difference on a card.
    let whole = Cut::Whole;
    assert_eq!(asks(WHOLE, STANDARD, on_card(1, whole)), 12_298_289_151);
    assert_eq!(asks(WHOLE, CORTEX, on_card(1, whole)), 12_298_289_151);
    // With the scores on the processor they are the less: 819 200 000 + 7 782 400 000 =
    // 8 601 600 000, and a quarter more is 10 752 000 000.
    let scored_there = scored_on_processor(1, whole);
    assert_eq!(asks(WHOLE, STANDARD, scored_there), 10_752_000_000);
    // On the processor the networks ask for 700 bytes a voxel, 2 867 200 000, which is less
    // than the scores: 819 200 000 + 3 328 599 654 = 4 147 799 654, and a quarter more is
    // 5 184 749 567.
    assert_eq!(asks(WHOLE, STANDARD, on_processor(whole)), 5_184_749_567);
    // With the cortex network, 900 bytes a voxel, 3 686 400 000, they are the more:
    // 819 200 000 + 3 686 400 000 = 4 505 600 000, and a quarter more is 5 632 000 000.
    assert_eq!(asks(WHOLE, CORTEX, on_processor(whole)), 5_632_000_000);
}

#[test]
fn the_scores_figure_is_for_where_the_scores_network_runs_not_for_the_networks() {
    // The networks on the processor and the scores on a card: the scores are the more, and
    // ask what they ask on a card, as in the first sum above.
    let strange = Rung {
        device: Device::Processor,
        cut: Cut::Whole,
        scores_on: Device::Card(id(1)),
    };
    assert_eq!(asks(WHOLE, STANDARD, strange), 12_298_289_151);
    // No voxels: only the scores network asks, 9 019 431 321 and a quarter more, and
    // 3 328 599 654 and a quarter more, which is what the processor needs of it.
    assert_eq!(asks(0, STANDARD, on_card(1, Cut::Whole)), 11_274_289_151);
    assert_eq!(asks(0, STANDARD, on_processor(Cut::Whole)), 4_160_749_567);
    // A count too large to multiply is as much as can be counted, not a wrong small figure.
    assert_eq!(asks(u64::MAX, STANDARD, on_card(1, Cut::Whole)), u64::MAX);
}

#[test]
fn what_a_run_asks_does_not_depend_on_which_card_or_on_the_robust_networks_or_the_cut() {
    let robust = Wanted {
        robust: true,
        cortex: false,
    };
    let (here, there) = (on_card(1, Cut::Whole), on_card(7, Cut::Head));
    assert_eq!(asks(WHOLE, STANDARD, here), asks(WHOLE, STANDARD, there));
    assert_eq!(asks(WHOLE, robust, here), asks(WHOLE, STANDARD, here));
    let processor = on_processor(Cut::Whole);
    assert_eq!(
        asks(WHOLE, robust, processor),
        asks(WHOLE, STANDARD, processor)
    );
}

#[test]
fn a_card_rung_goes_when_ordinary_memory_cannot_back_it_and_a_processor_rung_stays_if_it_fits() {
    let mut room = room();
    // The card has 8 GiB: room for the networks but not the scores network, which goes to the
    // processor. The computer has 9 GiB of ordinary memory to promise, and the whole scan on
    // the card asks for 10 752 000 000 bytes of it, 10.01 GiB. The head on the card asks for
    // 5 250 000 000 and the scan on the processor for 5 184 749 567 and 4 660 749 567.
    room.budgets = vec![None, Some(8 * GIB)];
    room.free_memory = 9 * GIB;
    // The rungs that are left keep the order they had.
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Automatic),
        vec![
            on_processor(Cut::Whole),
            scored_on_processor(1, Cut::Head),
            on_processor(Cut::Head),
        ]
    );
    // The card asked for by name is held to it as well.
    let named = rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(1)));
    assert_eq!(named, vec![scored_on_processor(1, Cut::Head)]);
}

#[test]
fn a_rung_is_kept_while_the_system_can_promise_what_it_asks_to_the_byte() {
    let mut room = room();
    // Card 1 has 15 GiB, which holds the networks and the scores network for either cut;
    // card 0 is the computer's own graphics. A card rung is tried with the scores network on
    // the card and then on the processor, so each of those is held to what it asks apart.
    let every = [
        (on_card(1, Cut::Whole), RunsOn::Card(id(1))),
        (on_card(1, Cut::Head), RunsOn::Card(id(1))),
        (scored_on_processor(1, Cut::Whole), RunsOn::Card(id(1))),
        (scored_on_processor(1, Cut::Head), RunsOn::Card(id(1))),
        (on_card(0, Cut::Whole), RunsOn::Card(id(0))),
        (on_card(0, Cut::Head), RunsOn::Card(id(0))),
        (scored_on_processor(0, Cut::Whole), RunsOn::Card(id(0))),
        (scored_on_processor(0, Cut::Head), RunsOn::Card(id(0))),
        (on_processor(Cut::Whole), RunsOn::Processor),
        (on_processor(Cut::Head), RunsOn::Processor),
    ];
    for (rung, runs_on) in every {
        let voxels = if rung.cut == Cut::Whole { WHOLE } else { HEAD };
        let asked = asks(voxels, STANDARD, rung);
        room.free_memory = asked;
        let found = rungs(WHOLE, HEAD, STANDARD, &room, runs_on);
        assert!(
            found.contains(&rung),
            "{rung:?} is not offered with {asked}"
        );
        room.free_memory = asked - 1;
        let found = rungs(WHOLE, HEAD, STANDARD, &room, runs_on);
        assert!(
            !found.contains(&rung),
            "{rung:?} is offered with {asked} - 1"
        );
    }
}

#[test]
fn a_card_with_room_for_the_scores_network_is_given_them_on_the_processor_when_that_is_all_that_fits()
 {
    let mut room = room();
    // Card 1 has 15 GiB, room for the networks and the scores network alike, and the computer
    // can promise 6 GiB. The head on the card with the scores network there asks for
    // 11 774 289 151 bytes of it, which is too much; with the scores network on the processor
    // it asks for 5 250 000 000, which fits. The whole scan asks for 12 298 289 151 or
    // 10 752 000 000, and the processor's whole scan for 5 184 749 567, which fits.
    room.free_memory = 6 * GIB;
    let head = scored_on_processor(1, Cut::Head);
    // The rungs that are left keep the order they had.
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Automatic),
        vec![on_processor(Cut::Whole), head, on_processor(Cut::Head)]
    );
    // The card asked for by name has the same: the head, with the scores network moved.
    let named = rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(1)));
    assert_eq!(named, vec![head]);
    // So has the computer's own graphics, which have the 6 GiB and so no room for the scores
    // network on them.
    let own = rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(0)));
    assert_eq!(own, vec![scored_on_processor(0, Cut::Head)]);
    // With just enough to back the head's scores network on the card, the head has it there,
    // and the whole scan, which asks for more, does not.
    room.free_memory = 11_774_289_151;
    let named = rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(1)));
    assert_eq!(
        named,
        vec![scored_on_processor(1, Cut::Whole), on_card(1, Cut::Head)]
    );
}

#[test]
fn when_neither_placement_of_the_scores_network_can_be_backed_the_rung_is_dropped_and_why_not_says_so()
 {
    let mut room = room();
    // The scores network fits the processor (3.875 GiB of the 4) and the card has room for
    // everything, but the smallest run asks for 5 250 000 000 bytes (4.89 GiB) with the
    // scores network on the processor and the networks on the card, and 4 660 749 567
    // (4.34 GiB) with all on the processor: more than the 4 GiB.
    room.free_memory = 4 * GIB;
    for runs_on in [
        RunsOn::Automatic,
        RunsOn::Card(id(1)),
        RunsOn::Card(id(0)),
        RunsOn::Processor,
    ] {
        let found = rungs(WHOLE, HEAD, STANDARD, &room, runs_on);
        assert_eq!(found, Vec::new(), "{runs_on:?}");
    }
    let said = why_not(WHOLE, HEAD, STANDARD, &room);
    assert!(said.contains("The computer has 4.0 GB of ordinary memory free."));
    let asked = "asks for about 4.9 GB of that when the networks are on a graphics card, and 4.4";
    assert!(said.contains(asked), "{said}");
}

#[test]
fn with_plenty_of_ordinary_memory_the_rungs_are_as_they_were() {
    let mut room = room();
    room.free_memory = 1_000 * GIB;
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Automatic),
        vec![
            on_card(1, Cut::Whole),
            on_processor(Cut::Whole),
            on_card(1, Cut::Head),
            on_processor(Cut::Head),
        ]
    );
    assert_eq!(
        rungs(WHOLE, HEAD, STANDARD, &room, RunsOn::Card(id(0))),
        vec![on_card(0, Cut::Whole), on_card(0, Cut::Head)]
    );
}

#[test]
fn with_too_little_ordinary_memory_for_any_run_nothing_is_offered_and_why_not_says_so() {
    let mut room = room();
    // The card has 15 GiB to spare and holds either cut. The computer can promise 2 GiB, and
    // the smallest run asks for 4 660 749 567 bytes of it, on the processor.
    room.free_memory = 2 * GIB;
    for runs_on in [RunsOn::Automatic, RunsOn::Card(id(1)), RunsOn::Processor] {
        let found = rungs(WHOLE, HEAD, STANDARD, &room, runs_on);
        assert_eq!(found, Vec::new(), "{runs_on:?}");
    }
    // The head is 2 000 000 voxels: it needs 3.5 GB (3.26 GiB) on a card and 1.75 GB
    // (1.63 GiB) on the processor, and the scores network 10.25 GiB and 3.875 GiB. It asks
    // for 5 250 000 000 bytes (4.89 GiB) of ordinary memory with the scores on the processor
    // and the networks on a card, and 4 660 749 567 (4.34 GiB) with all on the processor.
    // What a run needs is rounded up and what there is rounded down.
    assert_eq!(
        why_not(WHOLE, HEAD, STANDARD, &room),
        concat!(
            "There is not enough memory to label this scan. ",
            "At its smallest it needs about 3.3 GB on a graphics card or 1.7 GB on the ",
            "processor, and scoring the result needs about 10.3 GB on a graphics card or ",
            "3.9 GB on the processor. ",
            "The best graphics card has 15.0 GB to spare. ",
            "The computer has 2.0 GB of ordinary memory free. ",
            "The smallest run asks for about 4.9 GB of that when the networks are on a ",
            "graphics card, and 4.4 GB when they are on the processor."
        )
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
    // fit when they do not: the head needs 13.04 GB on a card and 6.52 on the processor, the
    // scores network 10.25 on a card and 3.88 on the processor, and the smallest run asks
    // for 19.56 GB of ordinary memory with the networks on a card and 8.38 on the processor.
    assert_eq!(
        why_not(whole, head, STANDARD, &room),
        concat!(
            "There is not enough memory to label this scan. ",
            "At its smallest it needs about 13.1 GB on a graphics card or 6.6 GB on the ",
            "processor, and scoring the result needs about 10.3 GB on a graphics card or ",
            "3.9 GB on the processor. ",
            "The best graphics card has 5.0 GB to spare. ",
            "The computer has 4.0 GB of ordinary memory free. ",
            "The smallest run asks for about 19.6 GB of that when the networks are on a ",
            "graphics card, and 8.4 GB when they are on the processor."
        )
    );
}

#[test]
fn why_not_says_only_what_is_true_of_a_computer_with_no_graphics_card() {
    let (whole, head) = (16_777_216, 8_000_000);
    let mut room = room();
    room.free_memory = 4 * GIB;
    room.drawing_on = None;
    room.budgets = vec![];
    // No card at all, and then software drawing in the place of one: nothing is said of a card
    // but that there is none. The head needs 6.52 GiB on the processor, the scores network
    // 3.875 GiB there, and the smallest run asks for 8.38 GiB of ordinary memory.
    let said = concat!(
        "There is not enough memory to label this scan. ",
        "At its smallest it needs about 6.6 GB on the processor, and scoring the result ",
        "needs about 3.9 GB on the processor. ",
        "There is no graphics card. ",
        "The computer has 4.0 GB of ordinary memory free. ",
        "The smallest run asks for about 8.4 GB of that on the processor."
    );
    room.cards = vec![];
    assert_eq!(why_not(whole, head, STANDARD, &room), said);
    room.cards = vec![card(2, CardKind::Software, 32 * GIB)];
    assert_eq!(why_not(whole, head, STANDARD, &room), said);
    // The computer's own graphics, which have no memory of their own, are said so.
    room.cards = vec![card(0, CardKind::Integrated, 128 * MIB)];
    let own = said.replace(
        "There is no graphics card. ",
        "There is no graphics card with memory of its own. ",
    );
    assert_eq!(why_not(whole, head, STANDARD, &room), own);
    // Nowhere in either does it speak of a card that is not there to speak of.
    for words in [said, own.as_str()] {
        assert!(!words.contains("on a graphics card"), "{words}");
    }
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
    room.drawing_on = Some(id(2));
    room.held = GIB / 2;
    room.free_memory = GIB + GIB / 4;
    let said = why_not(16_777_216, 16_777_216, CORTEX, &room);
    // Card 2 has 9 GiB less the half held there, which is 8.5 GB, and the software card is
    // not counted; the cortex network is the larger need on the processor.
    assert!(said.contains("The best graphics card has 8.5 GB to spare."));
    assert!(said.contains("The computer has 1.2 GB of ordinary memory free."));
    assert!(said.contains("about 27.4 GB on a graphics card or 17.6 GB on the processor"));

    room.cards = vec![card(0, CardKind::Integrated, 128 * MIB)];
    let said = why_not(16_777_216, 16_777_216, STANDARD, &room);
    assert!(said.contains("There is no graphics card with memory of its own."));
    assert!(!said.contains("to spare"));
}
