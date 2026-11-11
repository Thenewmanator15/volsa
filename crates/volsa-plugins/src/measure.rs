//! A plugin's memory, measured once and kept beside it, and where a run fits by it.
//!
//! Every input a plugin's network sees has a fixed size, so one run of one made-up block shows
//! what every run needs: the most ordinary memory the helper held, as its job object counts it,
//! and the most of a card it held. The figures are kept in `measured.json` beside the plugin's
//! copy, for the runtime they were measured with (by its sum) and each card (by its name). A
//! run goes only where the figures, with a quarter added, fit what the computer can still give;
//! and that same figure caps the helper's memory, so the system enforces it.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use volsa_models::{Card, CardKind, Device, Helper, Room, RunsOn, Stop, start_helper};

use crate::helper::{Client, HelperError, OpenJob};
use crate::watched::on_helper;

#[cfg(test)]
mod tests;

/// The name of the file the figures are kept in, beside the plugin's copy.
pub const MEASURED: &str = "measured.json";
/// The longest that file is read, in bytes.
const MOST_MEASURED: u64 = 1 << 16;
/// What Volsa holds for the statistics of a run beside a block and the labels: the histogram.
const STATISTICS: u64 = 2 << 20;

/// What a plugin was measured to need.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measured {
    /// The SHA-256 of the runtime's library the figures were measured with.
    pub runtime_sum: String,
    /// The most ordinary memory the helper held on the processor, in bytes, once measured.
    pub processor: Option<u64>,
    /// The figures of each card it was measured on.
    pub cards: Vec<CardFigures>,
}

/// What a plugin was measured to need on one card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardFigures {
    /// The card's name.
    pub name: String,
    /// The most of the card the helper held, in bytes.
    pub card_bytes: u64,
    /// The most ordinary memory the helper held, in bytes.
    pub ordinary_bytes: u64,
}

/// A place a run fits, and the cap on the helper's ordinary memory there. A cap of 0 is a
/// place the plugin has not been measured on: it is measured first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fits {
    /// Where the network runs.
    pub device: Device,
    /// The most ordinary memory the helper may hold, in bytes; 0 to measure first.
    pub cap: u64,
}

/// What starts a helper: the program itself, or a stand-in in a test.
pub trait HelperStarter: Send + Sync {
    /// Starts a helper, its ordinary memory capped at `cap` bytes when one is given.
    ///
    /// # Errors
    ///
    /// When the helper could not be started.
    fn start(&self, cap: Option<u64>) -> io::Result<Box<dyn HelperPipes>>;
}

/// A started helper: Volsa's end of its pipes, a way to end it, and the most it held.
pub trait HelperPipes: Send {
    /// Volsa's end of the pipes, the first time it is asked for.
    fn client(&mut self) -> Option<Client<Box<dyn Read + Send>, Box<dyn Write + Send>>>;
    /// Ends the helper at once.
    fn stop(&mut self);
    /// The most ordinary memory it held, as its job counts it.
    fn peak_memory(&self) -> Option<u64>;
}

/// Starts the helper program: Volsa itself, with `--plugin-helper`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessStarter {
    /// The program to start.
    pub program: PathBuf,
}

impl HelperStarter for ProcessStarter {
    fn start(&self, cap: Option<u64>) -> io::Result<Box<dyn HelperPipes>> {
        start_helper(&self.program, &["--plugin-helper"], cap)
            .map(|helper| Box::new(helper) as Box<dyn HelperPipes>)
    }
}

impl HelperPipes for Helper {
    fn client(&mut self) -> Option<Client<Box<dyn Read + Send>, Box<dyn Write + Send>>> {
        let (input, output) = self.take_pipes()?;
        Some(Client::new(Box::new(output), Box::new(input)))
    }

    fn stop(&mut self) {
        Helper::stop(self);
    }

    fn peak_memory(&self) -> Option<u64> {
        Helper::peak_memory(self)
    }
}

/// The figures kept beside a plugin, if there are any that can be read.
pub fn read_measured(folder: &Path) -> Option<Measured> {
    let file = fs::File::open(folder.join(MEASURED)).ok()?;
    let mut text = Vec::new();
    file.take(MOST_MEASURED).read_to_end(&mut text).ok()?;
    serde_json::from_slice(&text).ok()
}

/// Keeps the figures beside a plugin: written whole under another name, then put in place.
///
/// # Errors
///
/// When the file cannot be written.
pub fn write_measured(folder: &Path, measured: &Measured) -> io::Result<()> {
    let text = serde_json::to_vec_pretty(measured).map_err(io::Error::other)?;
    let part = folder.join(format!("{MEASURED}.part"));
    fs::write(&part, text)?;
    fs::rename(&part, folder.join(MEASURED))
}

/// Measures a plugin: one helper, capped at `cap` when one is given, runs the job on one
/// made-up block (a smooth pattern, not zeros) and ends, at once when `stop` is set. Gives the
/// most ordinary memory the helper held and the most of a card, when it ran on one.
///
/// # Errors
///
/// [`HelperError`] when the run failed: the plugin cannot run there.
pub fn measure(
    starter: &dyn HelperStarter,
    job: &OpenJob,
    cap: Option<u64>,
    stop: &Stop,
) -> Result<(u64, Option<u64>), HelperError> {
    let voxels: usize = job.block.iter().product();
    #[expect(clippy::cast_possible_truncation, reason = "a made-up pattern")]
    let block: Vec<f32> = (0..voxels)
        .map(|at| (at as f64 * 0.01).sin() as f32)
        .collect();
    let (outcome, peak) = on_helper(starter, cap, stop, |client| {
        client
            .open(job)
            .and_then(|()| client.block(&block))
            .and_then(|_| client.finish())
    })?;
    outcome.map(|card| (peak.unwrap_or(0), card))
}

/// What Volsa itself holds during a run beside the helper: one block of 32-bit numbers, the
/// grid of 16-bit labels, and the statistics.
pub fn own_bytes(block: [usize; 3], grid_voxels: u64) -> u64 {
    let block: u64 = block.iter().map(|side| *side as u64).product();
    block * 4 + grid_voxels * 2 + STATISTICS
}

/// A figure and a quarter more.
fn with_a_quarter(bytes: u64) -> u64 {
    bytes.saturating_add(bytes / 4)
}

/// Where a run of a plugin with these figures fits, in the order to try: with
/// [`RunsOn::Automatic`] each discrete card and then the processor; a card or the processor
/// alone when one was asked for. A card fits when what the helper held of it, with a quarter
/// added, is within its budget, and what it held is within what no program holds of it, where
/// those can be read; any place fits when the helper's ordinary memory with a quarter added,
/// and Volsa's own part, are within what the system can still promise. A place with no figures
/// is offered with a cap of 0, to be measured first.
pub fn places(measured: &Measured, own: u64, room: &Room, runs_on: RunsOn) -> Vec<Fits> {
    let fits_ordinary = |ordinary: u64| {
        let cap = with_a_quarter(ordinary);
        (cap.saturating_add(own) <= room.free_memory).then_some(cap)
    };
    let on_card = |place: usize, card: &Card| -> Option<Fits> {
        let device = Device::Card(card.id);
        let Some(figures) = measured
            .cards
            .iter()
            .find(|figures| figures.name == card.name)
        else {
            return Some(Fits { device, cap: 0 });
        };
        let ordinary = match card.kind {
            CardKind::Integrated => figures.ordinary_bytes.saturating_add(figures.card_bytes),
            CardKind::Discrete => {
                let budget = room.budgets.get(place).copied().flatten();
                let unheld = room.unheld.get(place).copied().flatten();
                let within_budget =
                    budget.is_none_or(|budget| with_a_quarter(figures.card_bytes) <= budget);
                let within_free = unheld.is_none_or(|unheld| figures.card_bytes <= unheld);
                if !(within_budget && within_free) {
                    return None;
                }
                figures.ordinary_bytes
            }
            CardKind::Software => return None,
        };
        fits_ordinary(ordinary).map(|cap| Fits { device, cap })
    };
    let processor = || match measured.processor {
        None => Some(Fits {
            device: Device::Processor,
            cap: 0,
        }),
        Some(ordinary) => fits_ordinary(ordinary).map(|cap| Fits {
            device: Device::Processor,
            cap,
        }),
    };
    let cards = room.cards.iter().enumerate();
    match runs_on {
        RunsOn::Automatic => cards
            .filter(|(_, card)| card.kind == CardKind::Discrete)
            .filter_map(|(place, card)| on_card(place, card))
            .chain(processor())
            .collect(),
        RunsOn::Card(id) => cards
            .filter(|(_, card)| card.id == id)
            .take(1)
            .filter_map(|(place, card)| on_card(place, card))
            .collect(),
        RunsOn::Processor => processor().into_iter().collect(),
    }
}
