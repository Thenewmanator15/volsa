//! The worker that labels a scan, on a graphics card that other programs have all but filled:
//! the system's budget for Volsa still says the card has room, and what no program holds of it
//! says not. The run goes to the processor, and its result says why. Stand-ins only (see
//! `labelling_stand_ins.rs`); every figure is made up.

use volsa_labels::Made;
use volsa_models::{Device, Role, RunsOn, needs};

use super::lab::{Lab, failed_run, made_by_run};
use super::stand_ins::{GIB, ROBUST, StandIn, card_a, head, one_at_a_time};

/// What the result of a run that a full card sent to the processor says of it.
const FULL: &str = "Run on the processor: the graphics card did not have room. Only 1.0 GB of \
                    its memory was free; programs were using the rest.";

/// A run on `runs_on`, on the made-up card with this budget, with 64 GiB of ordinary memory free
/// and this much of the card held by no program: who made the labels, and each network opened
/// and where.
fn run(runs_on: RunsOn, budget: u64, unheld: Option<u64>) -> (Made, Vec<(Role, Device)>) {
    let _alone = one_at_a_time();
    let lab = Lab::new(StandIn::with(budget, 64 * GIB).unheld(vec![unheld]));
    lab.label(1, runs_on);
    let (made, _) = made_by_run(&lab.until_the_end());
    (made, lab.stand_in.opened())
}

/// A run left to Volsa on the made-up card, whose budget is 16 GiB.
fn left_to_volsa(unheld: Option<u64>) -> (Made, Vec<(Role, Device)>) {
    run(RunsOn::Automatic, 16 * GIB, unheld)
}

#[test]
fn a_card_other_programs_have_filled_is_passed_over_for_the_processor_and_the_result_says_so() {
    let (made, opened) = left_to_volsa(Some(GIB));
    assert_eq!(made.device, "the processor");
    assert_eq!(made.notes, [FULL]);
    let nowhere_else = opened.iter().all(|(_, on)| *on == Device::Processor);
    assert!(nowhere_else, "the card is never tried: {opened:?}");
}

#[test]
fn a_card_its_budget_alone_rules_out_or_the_processor_chosen_by_hand_says_nothing_new() {
    // A card whose budget is too small whatever other programs hold: the run went to the
    // processor before any of this, and its result said nothing of the card. It still does not.
    for unheld in [None, Some(GIB / 2), Some(16 * GIB)] {
        let (made, _) = run(RunsOn::Automatic, GIB, unheld);
        assert_eq!(made.device, "the processor", "{unheld:?}");
        assert_eq!(made.notes, Vec::<String>::new(), "{unheld:?}");
    }
    // Nor does a run sent to the processor by hand, full card or not.
    let (made, _) = run(RunsOn::Processor, 16 * GIB, Some(GIB));
    assert_eq!(made.notes, Vec::<String>::new());
}

#[test]
fn a_card_whose_memory_is_free_or_not_known_is_used_as_before() {
    let on_the_card = Device::Card(card_a().id);
    for unheld in [Some(16 * GIB), None] {
        let (made, opened) = left_to_volsa(unheld);
        assert_eq!(made.device, "Made-up card A", "{unheld:?}");
        assert_eq!(made.notes, Vec::<String>::new(), "{unheld:?}");
        let all_there = opened.iter().all(|(_, on)| *on == on_the_card);
        assert!(all_there, "{unheld:?}: {opened:?}");
    }
}

#[test]
fn a_filled_card_chosen_by_hand_says_how_little_it_has_to_spare() {
    let _alone = one_at_a_time();
    let stand_in = StandIn::roomy().unheld(vec![Some(GIB)]);
    let lab = Lab::new(stand_in);
    lab.label(1, RunsOn::Card(card_a().id));
    let (_, why) = failed_run(&lab.until_the_end());
    assert!(
        why.contains("and the card has 1.0 GB to spare. It would fit if Volsa chose where"),
        "{why}"
    );
    assert_eq!(lab.stand_in.opened(), [], "nothing was tried");
}

#[test]
fn a_card_chosen_by_hand_with_free_memory_for_what_the_networks_take_is_said_to_have_room() {
    // What is free is held to what the networks take, with no quarter added: a byte less than
    // they need is room enough. The run is refused here for want of ordinary memory, and the
    // words say so, and not that the card is short.
    let _alone = one_at_a_time();
    let sizes = volsa_segment::sizes(&head()).expect("sizes");
    let smallest = sizes.whole.min(sizes.head);
    let need = needs(smallest, ROBUST, Device::Card(card_a().id));
    let stand_in = StandIn::of(vec![card_a()], vec![Some(16 * GIB)], 5 * GIB);
    let lab = Lab::new(stand_in.unheld(vec![Some(need - 1)]));
    lab.label(1, RunsOn::Card(card_a().id));
    let (_, why) = failed_run(&lab.until_the_end());
    assert!(
        why.contains(
            "The card has room for the networks that find the parts, but the computer's \
                      ordinary memory is short."
        ),
        "{why}"
    );
}

#[test]
fn a_card_with_room_for_the_networks_passed_over_for_ordinary_memory_adds_no_note() {
    // What is free holds what the networks take, so the card had room; the run went to the
    // processor because ordinary memory could not back it on the card, which no result has
    // spoken of. Its result does not say the card was short.
    let _alone = one_at_a_time();
    let sizes = volsa_segment::sizes(&head()).expect("sizes");
    let need = needs(sizes.whole, ROBUST, Device::Card(card_a().id));
    let stand_in = StandIn::of(vec![card_a()], vec![Some(16 * GIB)], 5 * GIB);
    let lab = Lab::new(stand_in.unheld(vec![Some(need - 1)]));
    lab.label(1, RunsOn::Automatic);
    let (made, _) = made_by_run(&lab.until_the_end());
    assert_eq!(made.device, "the processor");
    assert_eq!(made.notes, Vec::<String>::new());
}
