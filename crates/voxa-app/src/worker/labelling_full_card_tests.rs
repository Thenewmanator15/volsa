//! The worker that labels a scan, on a graphics card that other programs have all but filled:
//! the system's budget for Voxa still says the card has room, and what no program holds of it
//! says not. Stand-ins only (see `labelling_stand_ins.rs`); every figure is made up.

use voxa_labels::Made;
use voxa_models::{Device, Role, RunsOn, needs};

use super::lab::{Lab, failed_run, made_by_run};
use super::stand_ins::{GIB, ROBUST, StandIn, card_a, head, one_at_a_time};

/// A run left to Voxa on the made-up card, whose budget is 16 GiB, with 64 GiB of ordinary
/// memory free and this much of the card held by no program: who made the labels, and each
/// network opened and where.
fn left_to_voxa(unheld: Option<u64>) -> (Made, Vec<(Role, Device)>) {
    let _alone = one_at_a_time();
    let lab = Lab::new(StandIn::roomy().unheld(vec![unheld]));
    lab.label(1, RunsOn::Automatic);
    let (made, _) = made_by_run(&lab.until_the_end());
    (made, lab.stand_in.opened())
}

#[test]
fn a_card_other_programs_have_filled_is_passed_over_for_the_processor() {
    let (made, opened) = left_to_voxa(Some(GIB));
    assert_eq!(made.device, "the processor");
    let nowhere_else = opened.iter().all(|(_, on)| *on == Device::Processor);
    assert!(nowhere_else, "the card is never tried: {opened:?}");
}

#[test]
fn a_card_whose_memory_is_free_or_not_known_is_used_as_before() {
    let on_the_card = Device::Card(card_a().id);
    for unheld in [Some(16 * GIB), None] {
        let (made, opened) = left_to_voxa(unheld);
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
        why.contains("and the card has 1.0 GB to spare. It would fit if Voxa chose where"),
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
    let sizes = voxa_segment::sizes(&head()).expect("sizes");
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
