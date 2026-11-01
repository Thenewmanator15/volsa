use super::*;

const GIB: u64 = 1 << 30;

#[test]
fn the_default_budget_is_two_gigabytes_or_a_quarter_of_memory() {
    assert_eq!(default_for(Some(32 * GIB)), 2 << 30);
    assert_eq!(default_for(Some(4 * GIB)), 1 << 30);
    assert_eq!(
        default_for(None),
        2 << 30,
        "when the computer's memory is not known"
    );
}

#[test]
fn a_budget_set_is_taken_in_mebibytes_and_none_is_the_default() {
    assert_eq!(bytes_for(Some(512), Some(32 * GIB)), 512 << 20);
    assert_eq!(bytes_for(None, Some(4 * GIB)), 1 << 30);
}

#[test]
fn the_frame_cache_gets_what_the_volume_leaves_and_never_less_than_its_floor() {
    let budget = 512 << 20;
    assert_eq!(cache_share(budget, 0), budget);
    assert_eq!(cache_share(budget, 400 << 20), 112 << 20);
    assert_eq!(cache_share(budget, 500 << 20), CACHE_FLOOR);
    assert_eq!(
        cache_share(budget, 4 << 30),
        CACHE_FLOOR,
        "a volume past the budget"
    );
    assert_eq!(
        cache_share(1 << 20, 0),
        1 << 20,
        "a budget below the floor is all there is"
    );
}

#[test]
fn on_an_integrated_card_the_cards_copy_counts() {
    let held = Held {
        values: 300 << 20,
        cache: 100 << 20,
        card: 150 << 20,
    };
    assert_eq!(
        over(500 << 20, held, false),
        0,
        "a card with memory of its own"
    );
    assert_eq!(over(500 << 20, held, true), 50 << 20);
}
