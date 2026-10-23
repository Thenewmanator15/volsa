//! Which devices a run fits on, and in what order to try them. Given how many voxels a run
//! has, what is wanted of it and what the computer has, [`rungs`] lists the places a run can
//! be tried, best first, and [`why_not`] says why there are none.
//!
//! Nothing here asks the system anything. The card list, the budgets and the free memory all
//! come in through [`Room`], read by the caller at the moment it asks.

use crate::cards::{Card, CardId, CardKind};
use crate::network::Device;

mod words;

pub use words::{needed_gb, spare_gb, why_not};

#[cfg(test)]
mod ask_tests;
#[cfg(test)]
mod need_tests;
#[cfg(test)]
mod tests;

/// A GiB, in bytes. The figures below are in these, and so are the ones `why_not` says.
const GIB: u64 = 1 << 30;

/// What each network but the scores takes of a card's memory for every voxel of its grid, in
/// bytes. Measured on made-up volumes, not read from any network's file.
const ON_A_CARD: u64 = 1_400;
/// What each network but the scores and the cortex one asks the system for in ordinary memory
/// for every voxel of its grid, when it runs on the processor, in bytes. Measured on a made-up
/// head and rounded up.
const ON_THE_PROCESSOR: u64 = 700;
/// The same for the cortex network, the most of any network on the processor. Measured on a
/// made-up head and rounded up.
const CORTEX_ON_THE_PROCESSOR: u64 = 900;
/// What each network but the scores asks the system for in ordinary memory for every voxel of
/// its grid when it runs on a card, in bytes: Windows promises memory behind what a program
/// puts on a card. The cortex network asks for no more than the rest. Measured on a made-up
/// head and rounded up.
const ORDINARY_FOR_A_CARD: u64 = 1_900;
/// What Voxa itself holds beside any network for every voxel of its grid, in bytes, at its
/// most: the chances found, with, for a moment, the answer that has just arrived, which is
/// blended into them (no more than two sets of chances at once), and small lists. Measured on a
/// made-up head and rounded up.
const VOXA_ITSELF: u64 = 200;
/// What the scores network takes of a card's memory, whatever the size of the scan: 8.2 GiB.
/// Its input is always the same box. Measured on made-up volumes.
const SCORES_ON_A_CARD: u64 = 82 * GIB / 10;
/// What the scores network needs of a card: what it takes, and a quarter more.
const SCORES_NEED_ON_A_CARD: u64 = with_a_quarter(SCORES_ON_A_CARD);
/// What the scores network asks the system for in ordinary memory when it runs on the
/// processor, whatever the size of the scan: 3.1 GiB. Measured on a made-up head and rounded
/// up.
const SCORES_ON_THE_PROCESSOR: u64 = 31 * GIB / 10;
/// What the scores network needs of the processor: what it asks for, and a quarter more.
const SCORES_NEED_ON_THE_PROCESSOR: u64 = with_a_quarter(SCORES_ON_THE_PROCESSOR);
/// What the scores network asks the system for in ordinary memory when it runs on a card,
/// whatever the size of the scan: 8.4 GiB. Measured on a made-up head and rounded up.
const SCORES_ORDINARY_FOR_A_CARD: u64 = 84 * GIB / 10;

/// What is wanted of a run, as far as it changes what the run needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted {
    /// The three robust networks stand in for the standard one. Measured, they take no more
    /// than it does, on a card and on the processor, so this changes nothing in what a run
    /// needs or asks for.
    pub robust: bool,
    /// The cortex network runs too, to name the areas of the cortex. Measured, it asks the
    /// system for more than the others on the processor (900 bytes a voxel where they ask
    /// for 700), and no more than them on a card.
    pub cortex: bool,
}

/// Where the person has asked a run to go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunsOn {
    /// Wherever it fits best: each discrete card, then the processor.
    Automatic,
    /// On this card and no other: a [`Card`]'s `id`. An integrated card can be asked for; a
    /// software one cannot.
    Card(CardId),
    /// On the processor and nowhere else.
    Processor,
}

/// How much of the scan is given to the networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cut {
    /// The whole grid.
    Whole,
    /// The head only: the part of the grid that holds the head, which is smaller.
    Head,
}

/// One way to run, which fits: a place to try, in the order [`rungs`] gives them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rung {
    /// Where the networks that give the labels run.
    pub device: Device,
    /// How much of the scan they are given.
    pub cut: Cut,
    /// Where the scores network runs: on `device` when it fits there and the system can back
    /// the run so; otherwise on the processor, even with room for it on the card, when the
    /// ordinary memory could not back it there.
    pub scores_on: Device,
}

/// What the computer has, as the caller read it just now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    /// The graphics cards, as [`cards`](crate::cards()) lists them.
    pub cards: Vec<Card>,
    /// What the system lets this program use on each card, in bytes: [`budget`](crate::budget)
    /// of that card's `id`. It lies beside `cards`, in the same order. A `None`, or a place
    /// past the end, is a budget that could not be read.
    pub budgets: Vec<Option<u64>>,
    /// The card Voxa draws on, by its `id`, if it draws on one.
    pub drawing_on: Option<CardId>,
    /// How many bytes of that card Voxa itself holds, for drawing. It matters only when that
    /// card has no budget.
    pub held: u64,
    /// How many more bytes of ordinary memory the system can promise this program now: on
    /// Windows the commit still available. It is what a refused allocation runs into. The
    /// processor has it, and so does an integrated card, which shares it; and a run on any
    /// card asks the system for ordinary memory too (see [`asks`]).
    pub free_memory: u64,
}

/// What the figure needs, and a quarter more, which a run takes for the room it works in.
/// A figure too large to count is as much as can be counted.
const fn with_a_quarter(bytes: u64) -> u64 {
    bytes.saturating_add(bytes / 4)
}

/// What the largest labelling network asks the system for on the processor for every voxel
/// of its grid, in bytes.
const fn on_the_processor(wanted: Wanted) -> u64 {
    if wanted.cortex {
        CORTEX_ON_THE_PROCESSOR
    } else {
        ON_THE_PROCESSOR
    }
}

/// How many bytes the networks of a run need on a device, to give the labels of a grid of
/// `voxels`: what the largest of them takes for each voxel, and a quarter more. Measured on a
/// made-up head, that is 1 400 bytes of a card, and 700 of the processor's ordinary memory
/// (900 with the cortex network). The scores network is not counted, since it takes the same
/// whatever the grid; it is checked apart.
pub fn needs(voxels: u64, wanted: Wanted, device: Device) -> u64 {
    let a_voxel = match device {
        Device::Card(_) => ON_A_CARD,
        Device::Processor => on_the_processor(wanted),
    };
    with_a_quarter(voxels.saturating_mul(a_voxel))
}

/// What a whole run on this rung asks the system for in ordinary memory, at its peak: what
/// Voxa itself holds, and the larger of what its labelling networks and its scores network
/// ask for where each runs; a quarter added. `voxels` is the grid the rung's cut gives, as
/// [`rungs`] counts it. A run on a card asks for ordinary memory too, and more than one on
/// the processor does, since Windows promises memory behind what a program puts on a card.
pub fn asks(voxels: u64, wanted: Wanted, rung: Rung) -> u64 {
    let networks = match rung.device {
        Device::Card(_) => ORDINARY_FOR_A_CARD,
        Device::Processor => on_the_processor(wanted),
    };
    let scores = match rung.scores_on {
        Device::Card(_) => SCORES_ORDINARY_FOR_A_CARD,
        Device::Processor => SCORES_ON_THE_PROCESSOR,
    };
    let own = voxels.saturating_mul(VOXA_ITSELF);
    let largest = voxels.saturating_mul(networks).max(scores);
    with_a_quarter(own.saturating_add(largest))
}

/// A device a run might go on, and how many bytes it has for the run.
type Place = (Device, u64);

impl Room {
    /// How many bytes a device has for a run, as [`rungs`] counts them: what the system can
    /// still promise for the processor and for an integrated card, which shares it, and for a
    /// discrete card its budget, or its own memory less what Voxa holds of it. `None` for a
    /// card the computer does not list, and for software standing in for one, which is never
    /// a place to run.
    pub fn space_on(&self, device: Device) -> Option<u64> {
        match device {
            Device::Processor => Some(self.free_memory),
            Device::Card(id) => {
                let (place, card) = self.cards.iter().enumerate().find(|(_, c)| c.id == id)?;
                self.room_on(place, card)
            }
        }
    }

    /// How many bytes a discrete card has for a run. `place` is the card's place in `cards`.
    /// The system's budget already allows for what every program holds on the card, Voxa
    /// included. Where it could not be read, the card's own memory less what Voxa holds, if
    /// it is the card Voxa draws on.
    fn discrete_room(&self, place: usize, card: &Card) -> u64 {
        let held = if self.drawing_on == Some(card.id) {
            self.held
        } else {
            0
        };
        let budget = self.budgets.get(place).copied().flatten();
        budget.unwrap_or_else(|| card.own_bytes.saturating_sub(held))
    }

    /// How many bytes a card has for a run, or `None` for one that is never used: software
    /// standing in for a card.
    fn room_on(&self, place: usize, card: &Card) -> Option<u64> {
        match card.kind {
            CardKind::Discrete => Some(self.discrete_room(place, card)),
            CardKind::Integrated => Some(self.free_memory),
            CardKind::Software => None,
        }
    }

    /// The devices a run may go on, in the order to try them, each with what it has.
    fn places(&self, runs_on: RunsOn) -> Vec<Place> {
        let on_the_processor = (Device::Processor, self.free_memory);
        let cards = self.cards.iter().enumerate();
        match runs_on {
            RunsOn::Automatic => cards
                .filter(|(_, card)| card.kind == CardKind::Discrete)
                .map(|(place, card)| (Device::Card(card.id), self.discrete_room(place, card)))
                .chain([on_the_processor])
                .collect(),
            RunsOn::Card(id) => cards
                .filter(|(_, card)| card.id == id)
                .filter_map(|(place, card)| Some((Device::Card(id), self.room_on(place, card)?)))
                .take(1)
                .collect(),
            RunsOn::Processor => vec![on_the_processor],
        }
    }

    /// Where the scores network may run for a rung on `device`, which has `space` bytes, in
    /// the order to try: there when it fits there, then on the processor when that much is
    /// free. Empty when it fits in neither place.
    fn scores_places(&self, device: Device, space: u64) -> Vec<Device> {
        let there = match device {
            Device::Card(_) => SCORES_NEED_ON_A_CARD,
            Device::Processor => SCORES_NEED_ON_THE_PROCESSOR,
        };
        let mut places = Vec::new();
        if there <= space {
            places.push(device);
        }
        if device != Device::Processor && SCORES_NEED_ON_THE_PROCESSOR <= self.free_memory {
            places.push(Device::Processor);
        }
        places
    }
}

/// The ways to run a scan of `whole` voxels, or of `head` voxels cut to the head, that fit,
/// in the order to try them. A rung is offered only when the networks fit on its device, the
/// scores network fits somewhere, and the system can still promise the ordinary memory the
/// whole run asks for ([`asks`]), on a card as on the processor. The scores network goes on
/// the rung's own device when it fits there and the system can back the run so; failing that
/// it goes on the processor, which asks the less of ordinary memory, if the system can back
/// the run so. A rung that is held back for want of that leaves the order of the others as it
/// was. An answer over 2 GiB is no bar: it is fetched by two runs.
///
/// With [`RunsOn::Automatic`] the order is the whole scan on each discrete card, then on the
/// processor, then the head on each discrete card, then on the processor. An integrated card
/// is not tried unless it is asked for. A named card, or the processor, is offered the whole
/// scan and then the head, and nowhere else. The head is offered only where cutting to it
/// makes the grid smaller: otherwise it would be the same run tried again.
pub fn rungs(whole: u64, head: u64, wanted: Wanted, room: &Room, runs_on: RunsOn) -> Vec<Rung> {
    let places = room.places(runs_on);
    let mut found = Vec::new();
    let cuts = [(Cut::Whole, whole), (Cut::Head, head)];
    for (cut, voxels) in cuts
        .into_iter()
        .filter(|&(cut, voxels)| cut == Cut::Whole || voxels < whole)
    {
        for &(device, space) in &places {
            if needs(voxels, wanted, device) > space {
                continue;
            }
            let tries = room
                .scores_places(device, space)
                .into_iter()
                .map(|scores_on| Rung {
                    device,
                    cut,
                    scores_on,
                });
            // The first place for the scores network that the system can back.
            found.extend(
                tries
                    .filter(|&rung| asks(voxels, wanted, rung) <= room.free_memory)
                    .take(1),
            );
        }
    }
    found
}
