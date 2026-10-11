//! Which devices a run fits on, and in what order to try them. Given how many voxels a run
//! has, what is wanted of it and what the computer has, [`rungs`] lists the places a run can
//! be tried, best first, and [`why_not`] says why there are none.
//!
//! Nothing here asks the system anything. The card list, the budgets and the free memory all
//! come in through [`Room`], read by the caller at the moment it asks.

use crate::cards::{Card, CardId, CardKind};
use crate::network::Device;

#[cfg(test)]
mod need_tests;
#[cfg(test)]
mod tests;

/// A GiB, in bytes. The figures below are in these, and so are the ones `why_not` says.
const GIB: u64 = 1 << 30;

/// What each network but the scores takes of a card's memory for every voxel of its grid, in
/// bytes. Measured on made-up volumes, not read from any network's file.
const ON_A_CARD: u64 = 1_400;
/// What each network but the scores and the cortex one takes of the processor's memory for
/// every voxel of its grid, in bytes. Measured on made-up volumes.
const ON_THE_PROCESSOR: u64 = 2_000;
/// What the cortex network takes of the processor's memory for every voxel, in bytes: the
/// most of any network. Measured on made-up volumes.
const CORTEX_ON_THE_PROCESSOR: u64 = 2_300;
/// What the scores network takes on any device, whatever the size of the scan: 8.2 GiB. Its
/// input is always the same box. Measured on made-up volumes.
const SCORES: u64 = 82 * GIB / 10;
/// What the scores network needs to be given: what it takes, and a quarter more.
const SCORES_NEED: u64 = with_a_quarter(SCORES);

/// What is wanted of a run, as far as it changes what the run needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted {
    /// The three robust networks stand in for the standard one. Measured, they take no more
    /// than it does, so this changes nothing in what a run needs.
    pub robust: bool,
    /// The cortex network runs too, to name the areas of the cortex. It is the largest of the
    /// networks on the processor.
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
    /// Where the scores network runs: on `device` when it fits there, otherwise on the
    /// processor.
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
    /// How many bytes of ordinary memory are free: what the processor has, and what an
    /// integrated card shares.
    pub free_memory: u64,
}

/// What the figure needs, and a quarter more, which a run takes for the room it works in.
/// A figure too large to count is as much as can be counted.
const fn with_a_quarter(bytes: u64) -> u64 {
    bytes.saturating_add(bytes / 4)
}

/// How many bytes the networks of a run need on a device, to give the labels of a grid of
/// `voxels`: what the largest of them takes for each voxel, and a quarter more. The scores
/// network is not counted, since it takes the same whatever the grid; it is checked apart.
pub fn needs(voxels: u64, wanted: Wanted, device: Device) -> u64 {
    let a_voxel = match device {
        Device::Card(_) => ON_A_CARD,
        Device::Processor if wanted.cortex => CORTEX_ON_THE_PROCESSOR,
        Device::Processor => ON_THE_PROCESSOR,
    };
    with_a_quarter(voxels.saturating_mul(a_voxel))
}

/// A device a run might go on, and how many bytes it has for the run.
type Place = (Device, u64);

impl Room {
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

    /// Where the scores network runs for a rung on `device`, which has `space` bytes: there
    /// when it fits, otherwise on the processor when that much is free. `None` when it fits
    /// in neither place.
    fn scores_on(&self, device: Device, space: u64) -> Option<Device> {
        if SCORES_NEED <= space {
            Some(device)
        } else if SCORES_NEED <= self.free_memory {
            Some(Device::Processor)
        } else {
            None
        }
    }
}

/// The ways to run a scan of `whole` voxels, or of `head` voxels cut to the head, that fit,
/// in the order to try them. A rung is offered only when the networks fit on its device and
/// the scores network fits somewhere. An answer over 2 GiB is no bar: it is fetched by two
/// runs.
///
/// With [`RunsOn::Automatic`] the order is the whole scan on each discrete card, then on the
/// processor, then the head on each discrete card, then on the processor. An integrated card
/// is not tried unless it is asked for. A named card, or the processor, is offered the whole
/// scan and then the head, and nowhere else.
pub fn rungs(whole: u64, head: u64, wanted: Wanted, room: &Room, runs_on: RunsOn) -> Vec<Rung> {
    let places = room.places(runs_on);
    let mut found = Vec::new();
    for (cut, voxels) in [(Cut::Whole, whole), (Cut::Head, head)] {
        for &(device, space) in &places {
            if needs(voxels, wanted, device) > space {
                continue;
            }
            if let Some(scores_on) = room.scores_on(device, space) {
                found.push(Rung {
                    device,
                    cut,
                    scores_on,
                });
            }
        }
    }
    found
}

/// The gigabytes of a figure, to one place, rounded up: what is needed is never said to be
/// less than it is. A GB is a GiB, as the system shows memory.
fn needed_gb(bytes: u64) -> String {
    let tenths = (u128::from(bytes) * 10).div_ceil(u128::from(GIB));
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// The gigabytes of a figure, to one place, rounded down: what there is is never said to be
/// more than it is.
fn spare_gb(bytes: u64) -> String {
    let tenths = u128::from(bytes) * 10 / u128::from(GIB);
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// Why [`rungs`] gave none, in words fit to show: what the smallest cut needs, what the
/// scores network needs, and what the best card and the computer have. Rounded so that the
/// figures never seem to fit when they do not. It describes the computer, not a card asked
/// for by name.
pub fn why_not(whole: u64, head: u64, wanted: Wanted, room: &Room) -> String {
    let smallest = whole.min(head);
    // Which card makes no difference to what a run needs; only that it is a card.
    let on_a_card = needs(smallest, wanted, Device::Card(CardId(0)));
    let on_the_processor = needs(smallest, wanted, Device::Processor);
    let best_card = room
        .cards
        .iter()
        .enumerate()
        .filter(|(_, card)| card.kind == CardKind::Discrete)
        .map(|(place, card)| room.discrete_room(place, card))
        .max();
    let card = match best_card {
        Some(bytes) => format!(
            "The best graphics card has {} GB to spare.",
            spare_gb(bytes)
        ),
        None => "There is no graphics card with memory of its own.".to_owned(),
    };
    format!(
        "There is not enough memory to label this scan. \
         At its smallest it needs about {} GB on a graphics card or {} GB on the processor, \
         and scoring the result needs about {} GB on either. \
         {card} The computer has {} GB of ordinary memory free.",
        needed_gb(on_a_card),
        needed_gb(on_the_processor),
        needed_gb(SCORES_NEED),
        spare_gb(room.free_memory),
    )
}
