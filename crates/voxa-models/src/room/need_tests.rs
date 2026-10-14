//! Tests of what a run needs of a device. Every volume and card here is made up; the bytes a
//! voxel are the module's own, measured on made-up volumes.

use super::tests::{CORTEX, GIB, HEAD, STANDARD, WHOLE, card, id, on_processor, room};
use super::{Cut, RunsOn, Wanted, needed_gb, needs, rungs, spare_gb};
use crate::cards::CardKind;
use crate::network::Device;

const ROBUST: Wanted = Wanted {
    robust: true,
    cortex: false,
};

#[test]
fn a_run_needs_its_largest_network_per_voxel_with_a_quarter_added() {
    assert_eq!(
        needs(WHOLE, STANDARD, Device::Card(id(1))),
        4_096_000 * 1_400 * 5 / 4
    );
    assert_eq!(
        needs(WHOLE, STANDARD, Device::Processor),
        4_096_000 * 700 * 5 / 4
    );
    // The cortex network is the largest on the processor and no larger than the rest on a card.
    assert_eq!(
        needs(WHOLE, CORTEX, Device::Processor),
        4_096_000 * 900 * 5 / 4
    );
    assert_eq!(
        needs(WHOLE, CORTEX, Device::Card(id(1))),
        needs(WHOLE, STANDARD, Device::Card(id(1)))
    );
    // The robust networks need no more than the standard one, on a card and on the processor.
    assert_eq!(
        needs(WHOLE, ROBUST, Device::Processor),
        needs(WHOLE, STANDARD, Device::Processor)
    );
    assert_eq!(
        needs(WHOLE, ROBUST, Device::Card(id(1))),
        needs(WHOLE, STANDARD, Device::Card(id(1)))
    );
    // The card a run goes to makes no difference to its need, only the kind of device.
    assert_eq!(
        needs(WHOLE, STANDARD, Device::Card(id(0))),
        needs(WHOLE, STANDARD, Device::Card(id(7)))
    );
    assert_eq!(needs(0, STANDARD, Device::Processor), 0);
    // A count too large to multiply is as much as can be counted, not a wrong small figure.
    assert_eq!(needs(u64::MAX, STANDARD, Device::Processor), u64::MAX);
}

#[test]
fn the_cortex_network_asks_more_of_the_processor() {
    // The whole scan asks the system for 5 184 749 567 bytes with the standard network and
    // 5 632 000 000 with the cortex network, as `asks` says; the head asks for less.
    let mut room = room();
    room.free_memory = 5_400_000_000;
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
fn a_device_has_what_a_run_counts_on_there_or_none_when_it_is_no_place_to_run() {
    let mut room = room();
    // The processor has the ordinary memory free; so does a card that shares it.
    assert_eq!(room.space_on(Device::Processor), Some(20 * GIB));
    assert_eq!(room.space_on(Device::Card(id(0))), Some(20 * GIB));
    // A discrete card with no budget read has its own memory less what Voxa holds of it.
    assert_eq!(room.space_on(Device::Card(id(1))), Some(15 * GIB));
    room.budgets = vec![None, Some(6 * GIB)];
    assert_eq!(room.space_on(Device::Card(id(1))), Some(6 * GIB));
    // A card the computer does not list, and software standing in for one, are no places.
    assert_eq!(room.space_on(Device::Card(id(7))), None);
    room.cards.push(card(2, CardKind::Software, 0));
    room.budgets.push(None);
    assert_eq!(room.space_on(Device::Card(id(2))), None);
}

#[test]
fn the_gigabytes_said_are_a_gibibyte_each_needs_rounded_up_and_what_there_is_rounded_down() {
    assert_eq!(needed_gb(GIB), "1.0");
    assert_eq!(needed_gb(GIB + 1), "1.1");
    assert_eq!(needed_gb(0), "0.0");
    assert_eq!(spare_gb(GIB), "1.0");
    assert_eq!(spare_gb(GIB - 1), "0.9");
    assert_eq!(spare_gb(GIB / 10), "0.0");
    assert_eq!(spare_gb(3 * GIB + GIB / 2), "3.5");
}
