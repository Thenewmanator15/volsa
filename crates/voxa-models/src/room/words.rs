//! The words for what a run needs and what the computer has: figures in gigabytes, rounded so
//! that they never seem to fit when they do not, and why no run fits.

use super::{Cut, GIB, Room, Rung, SCORES_NEED_ON_A_CARD, SCORES_NEED_ON_THE_PROCESSOR, Wanted};
use super::{asks, needs};
use crate::cards::{CardId, CardKind};
use crate::network::Device;

/// The gigabytes of a figure, to one place, rounded up: what is needed is never said to be
/// less than it is. A GB is a GiB, as the system shows memory.
pub fn needed_gb(bytes: u64) -> String {
    let tenths = (u128::from(bytes) * 10).div_ceil(u128::from(GIB));
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// The gigabytes of a figure, to one place, rounded down: what there is is never said to be
/// more than it is. A GB is a GiB, as the system shows memory.
pub fn spare_gb(bytes: u64) -> String {
    let tenths = u128::from(bytes) * 10 / u128::from(GIB);
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// What a computer without a discrete card has to say of its graphics: none at all, or some
/// that have no memory of their own.
fn no_card(room: &Room) -> &'static str {
    let shares = room
        .cards
        .iter()
        .any(|card| card.kind == CardKind::Integrated);
    if shares {
        "There is no graphics card with memory of its own."
    } else {
        "There is no graphics card."
    }
}

/// Why [`rungs`](super::rungs) gave none, in words fit to show: what the smallest cut
/// needs, what the scores network needs, what the best card and the computer have, and how
/// much of the computer's ordinary memory the smallest run asks for. Rounded so that the
/// figures never seem to fit when they do not. It describes the computer, not a card asked
/// for by name. Only a discrete card is spoken of as a place to run: without one, the
/// processor is the only place there is, and the words say so.
pub fn why_not(whole: u64, head: u64, wanted: Wanted, room: &Room) -> String {
    let smallest = whole.min(head);
    let cut = if head < whole { Cut::Head } else { Cut::Whole };
    // The least a run asks of ordinary memory has the scores network on the processor.
    let asked = |device| {
        let scores_on = Device::Processor;
        let rung = Rung {
            device,
            cut,
            scores_on,
        };
        needed_gb(asks(smallest, wanted, rung))
    };
    let on_the_processor = needed_gb(needs(smallest, wanted, Device::Processor));
    let scores_there = needed_gb(SCORES_NEED_ON_THE_PROCESSOR);
    let asked_there = asked(Device::Processor);
    let best_card = room
        .cards
        .iter()
        .enumerate()
        .filter(|(_, card)| card.kind == CardKind::Discrete)
        .map(|(place, card)| room.discrete_room(place, card).spare())
        .max();
    // Which card makes no difference to what a run needs; only that it is a card.
    let on_a_card = Device::Card(CardId(0));
    let (needing, scoring, asking, card) = match best_card {
        Some(bytes) => (
            format!(
                "{} GB on a graphics card or {on_the_processor} GB on the processor",
                needed_gb(needs(smallest, wanted, on_a_card))
            ),
            format!(
                "{} GB on a graphics card or {scores_there} GB on the processor",
                needed_gb(SCORES_NEED_ON_A_CARD)
            ),
            format!(
                "{} GB of that when the networks are on a graphics card, and {asked_there} GB \
                 when they are on the processor",
                asked(on_a_card)
            ),
            format!(
                "The best graphics card has {} GB to spare.",
                spare_gb(bytes)
            ),
        ),
        None => (
            format!("{on_the_processor} GB on the processor"),
            format!("{scores_there} GB on the processor"),
            format!("{asked_there} GB of that on the processor"),
            no_card(room).to_owned(),
        ),
    };
    format!(
        "There is not enough memory to label this scan. \
         At its smallest it needs about {needing}, and scoring the result needs about \
         {scoring}. {card} The computer has {} GB of ordinary memory free. \
         The smallest run asks for about {asking}.",
        spare_gb(room.free_memory),
    )
}
