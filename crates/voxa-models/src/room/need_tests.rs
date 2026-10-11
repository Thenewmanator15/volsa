//! Tests of what a run needs of a device. Every volume and card here is made up; the bytes a
//! voxel are the module's own, measured on made-up volumes.

use super::tests::{CORTEX, HEAD, STANDARD, WHOLE, id, on_processor, room};
use super::{Cut, RunsOn, Wanted, needs, rungs};
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
        4_096_000 * 2_000 * 5 / 4
    );
    // The cortex network is the largest on the processor and no larger than the rest on a card.
    assert_eq!(
        needs(WHOLE, CORTEX, Device::Processor),
        4_096_000 * 2_300 * 5 / 4
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
