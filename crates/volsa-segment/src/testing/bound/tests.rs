//! Tests of the bound a run on the real networks is held to. The sums are worked out by hand
//! from the figures of `volsa-models`: Volsa's own 200 bytes a voxel, the scores network's
//! 3.1 GiB (3 328 599 654 bytes) on the processor and 8.4 GiB (9 019 431 321) on a card, and
//! a quarter more.

use volsa_models::{CardId, Device, Wanted};

use super::{Bound, shares_memory};

const STANDARD: Wanted = Wanted {
    robust: false,
    cortex: false,
};

/// A made-up image of 100 voxels, beside which the test holds an array of 10 numbers and one
/// of 5, which are 60 bytes.
fn bound() -> Bound {
    Bound::new(100, STANDARD, &[("the first", 10), ("the second", 5)])
}

#[test]
fn a_run_is_allowed_what_asks_says_and_what_the_test_holds_beside_it() {
    // Processor: 100 x 200 = 20 000 of Volsa's own, and the scores network's 3 328 599 654 is
    // more than the networks' 100 x 700 = 70 000; the sum 3 328 619 654 and a quarter more
    // is 4 160 774 567.
    assert_eq!(bound().allowed(Device::Processor), 4_160_774_567 + 60);
    // A card: the scores network's 9 019 431 321 is more than the networks' 100 x 1 900 =
    // 190 000; the sum 9 019 451 321 and a quarter more is 11 274 314 151.
    let card = Device::Card(CardId(7));
    assert_eq!(bound().allowed(card), 11_274_314_151 + 60);
    // Nothing beside it: the bound is what a run is allowed.
    let alone = Bound::new(100, STANDARD, &[]);
    assert_eq!(alone.allowed(Device::Processor), 4_160_774_567);
}

#[test]
fn a_run_is_held_to_the_bound_to_the_byte_and_the_sum_is_said_when_it_is_over() {
    let (device, allowed) = (Device::Processor, bound().allowed(Device::Processor));
    let said = bound()
        .held(device, Some(allowed), false)
        .expect("at the bound");
    assert!(said.contains(&allowed.to_string()), "{said}");
    let over = bound()
        .held(device, Some(allowed + 1), false)
        .expect_err("one over");
    let words = [
        "4160774628",
        "4160774627",
        "4160774567",
        "60",
        "the first 40",
        "the second 20",
    ];
    for word in words {
        assert!(over.contains(word), "`{word}` is not in: {over}");
    }
}

#[test]
fn integrated_graphics_are_shown_the_comparison_and_never_held_to_it() {
    let device = Device::Processor;
    let over = bound().allowed(device) + 1;
    let said = bound()
        .held(device, Some(over), true)
        .expect("shown, not held");
    assert!(said.contains(&over.to_string()), "{said}");
    assert!(bound().held(device, None, true).is_ok());
}

#[test]
fn a_count_that_was_not_read_is_a_failure_where_the_system_keeps_one() {
    let read = bound().held(Device::Processor, None, false);
    // Windows keeps the count, so that it cannot be read is a failure to measure; elsewhere
    // the system keeps none, and it is said so.
    assert_eq!(read.is_err(), cfg!(windows), "{read:?}");
}

#[test]
fn no_card_that_the_system_does_not_list_shares_the_computers_memory() {
    assert!(!shares_memory(Device::Processor));
    assert!(!shares_memory(Device::Card(CardId(u64::MAX))));
}
