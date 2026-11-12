//! Support for the tests that are given a runtime and a model folder: the real networks
//! behind [`Networks`], what a piece of labelling with them took, how far two sets of labels
//! agree, and the most a run may ask the system for ([`Bound`]). Nothing here reads a model
//! folder or loads the runtime by itself: a test hands both over, and says which device the
//! networks go on.
//!
//! A graphics card is named by what it is, its [`CardId`](volsa_models::CardId), and the
//! runtime finds where it stands at each opening. A place in the system's list is never kept
//! here: the list has changed its order between two networks of one run.

use std::collections::BTreeMap;
use std::fmt;
use std::process::{self, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::time::{Duration, Instant};
use std::{fs, thread};

use volsa_models::{
    Card, CardKind, Device, Model, Network, Role, RunError, Runtime, Stop, cards, in_use,
};

use crate::find::{Networks, Stage};

mod bound;

pub use bound::Bound;

/// How often a card is asked how much of it this process holds.
const LOOK_EVERY: Duration = Duration::from_millis(20);
/// A GiB, in bytes.
const GIB: f64 = 1_073_741_824.0;

/// The networks of a model folder, each opened on one device through the runtime.
#[derive(Debug, Clone)]
pub struct OnDevice {
    /// The runtime, loaded.
    pub runtime: Runtime,
    /// The model folder, read and checked.
    pub model: Model,
    /// Where every network is opened.
    pub device: Device,
    /// Ends a run in progress when it is set.
    pub stop: Stop,
}

impl OnDevice {
    /// The networks of a model folder on a device, with nothing set to stop them.
    pub fn new(runtime: &Runtime, model: &Model, device: Device) -> Self {
        Self {
            runtime: runtime.clone(),
            model: model.clone(),
            device,
            stop: Stop::new(),
        }
    }
}

impl Networks for OnDevice {
    fn open(&mut self, role: Role, sizes: [usize; 3]) -> Result<Box<dyn Network>, RunError> {
        let file = self.model.file(role);
        self.runtime.open(file, self.device, sizes, &self.stop)
    }
}

/// The graphics cards of one kind, each once: the system's list can hold a card twice. Two
/// entries are one card when they are the same card to the system, or have one name.
pub fn cards_of(kind: CardKind) -> Vec<Card> {
    once_each(cards(), kind)
}

/// Those of the cards listed that are of one kind, but for any that an earlier one already
/// stands for.
fn once_each(listed: Vec<Card>, kind: CardKind) -> Vec<Card> {
    let mut found: Vec<Card> = Vec::new();
    for card in listed {
        let again = |seen: &Card| seen.id == card.id || seen.name == card.name;
        if card.kind == kind && !found.iter().any(again) {
            found.push(card);
        }
    }
    found
}

/// The quickest device this computer has, with a name for it: a discrete card, or else the
/// processor.
pub fn quickest() -> (Device, String) {
    let card = cards_of(CardKind::Discrete).into_iter().next();
    let processor = (Device::Processor, "the processor".to_owned());
    card.map_or(processor, |card| (Device::Card(card.id), card.name))
}

/// What a piece of labelling took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Took {
    /// In words: the seconds in all and from each stage to the next, the most memory the
    /// process has had since it began, by the system's own counts, and for a card the most
    /// of the card this process was seen to hold, which is sampled.
    pub words: String,
    /// The most the process had asked the system for by the end, in bytes, since it began.
    /// `None` where the system keeps no such count, or it could not be read.
    pub asked: Option<u64>,
    /// For a card: by how many bytes what this process holds on it rose at the most while
    /// the work went on, over what it held just before. `None` on the processor.
    pub card_rose: Option<u64>,
}

/// Does a piece of labelling with these networks, and says what it took: see [`Took`]. The
/// stages are those that `work` announces.
pub fn measured<T>(
    networks: &mut OnDevice,
    work: impl FnOnce(&mut OnDevice, &mut dyn FnMut(Stage)) -> T,
) -> (T, Took) {
    let (device, started, mut stages) = (networks.device, Instant::now(), Vec::new());
    let before = match device {
        Device::Card(card) => in_use(card).unwrap_or(0),
        Device::Processor => 0,
    };
    let mut said = |stage: Stage| stages.push((stage, started.elapsed()));
    let (answer, of_the_card) = watched(device, || work(networks, &mut said));
    let ended = started.elapsed();
    let seconds = |(place, &(stage, from)): (usize, &(Stage, Duration))| {
        let until = stages.get(place + 1).map_or(ended, |next| next.1);
        let lasted = until.saturating_sub(from).as_secs_f64();
        format!("{stage:?} {lasted:.2} s")
    };
    let each: Vec<String> = stages.iter().enumerate().map(seconds).collect();
    let (all, each) = (ended.as_secs_f64(), each.join(", "));
    let peak = process_peak();
    let process = match peak {
        Some((held, Some(asked))) => format!(
            "{} held at once and {} asked for (the system's counts)",
            gib(held),
            gib(asked)
        ),
        Some((held, None)) => format!("{} held at once (the system's count)", gib(held)),
        None => "an amount that was not read".to_owned(),
    };
    let card = of_the_card.map(|most| format!("; of the card {} (sampled)", gib(most)));
    let card = card.unwrap_or_default();
    let took = Took {
        words: format!("{all:.1} s in all ({each}); most memory: of the process {process}{card}"),
        asked: peak.and_then(|(_, asked)| asked),
        card_rose: of_the_card.map(|most| most.saturating_sub(before)),
    };
    (answer, took)
}

/// A count of bytes, in GiB.
fn gib(bytes: u64) -> String {
    format!("{:.2} GiB", bytes as f64 / GIB)
}

/// The most memory this process has had, by the system's own counts, in bytes: the most it
/// held in memory at one time and, where the system keeps that count too, the most it had
/// asked for. `None` where neither can be read.
fn process_peak() -> Option<(u64, Option<u64>)> {
    if cfg!(windows) {
        let id = process::id();
        let ask = format!(
            "$p = Get-Process -Id {id}; \"$($p.PeakWorkingSet64) $($p.PeakPagedMemorySize64)\""
        );
        let mut asked = Command::new("powershell");
        asked.args(["-NoProfile", "-NonInteractive", "-Command", &ask]);
        let told = asked.output().ok()?;
        let told = String::from_utf8_lossy(&told.stdout);
        let mut counts = told.split_whitespace().map(str::parse::<u64>);
        let (held, asked_for) = (counts.next()?.ok()?, counts.next()?.ok()?);
        Some((held, Some(asked_for)))
    } else {
        let status = fs::read_to_string("/proc/self/status").ok()?;
        let most = status
            .lines()
            .find_map(|line| line.strip_prefix("VmHWM:"))?;
        let kilobytes: u64 = most.trim().trim_end_matches("kB").trim().parse().ok()?;
        Some((kilobytes.checked_mul(1024)?, None))
    }
}

/// Does some work and, when it is on a card, looks meanwhile at how much of the card this
/// process holds: the most that was seen, in bytes.
fn watched<T>(device: Device, work: impl FnOnce() -> T) -> (T, Option<u64>) {
    /// Tells the watcher to end, also when the work panics.
    struct Done<'a>(&'a AtomicBool);
    impl Drop for Done<'_> {
        fn drop(&mut self) {
            self.0.store(true, Relaxed);
        }
    }
    let Device::Card(card) = device else {
        return (work(), None);
    };
    let (done, most) = (AtomicBool::new(false), AtomicU64::new(0));
    let answer = thread::scope(|scope| {
        scope.spawn(|| {
            while !done.load(Relaxed) {
                most.fetch_max(in_use(card).unwrap_or(0), Relaxed);
                thread::sleep(LOOK_EVERY);
            }
        });
        let _done = Done(&done);
        work()
    });
    (answer, Some(most.into_inner()))
}

/// How far two sets of labels on one grid agree, measured as the converter's own check
/// measures it (`agreement` in `tools/synthseg-to-onnx/check.py`).
#[derive(Debug, Clone, PartialEq)]
pub struct Agreement {
    /// How many voxels the grid has.
    pub voxels: usize,
    /// How many of them have one label in the first set and another in the second.
    pub differ: usize,
    /// For every label but 0 that either set has, in the labels' order: the label, how many
    /// voxels have it in the first set, and its Dice. That is twice the voxels that have it
    /// in both sets, over those that have it in the first added to those in the second.
    pub dice: Vec<(u32, u64, f64)>,
}

impl Agreement {
    /// Measures two sets of labels against each other. Where one is longer than the other,
    /// what has no partner is left out.
    pub fn of(first: &[u32], second: &[u32]) -> Self {
        // For each label: the voxels that have it in the first set, in the second, in both.
        let mut counts: BTreeMap<u32, [u64; 3]> = BTreeMap::new();
        let mut differ = 0;
        for (&first, &second) in first.iter().zip(second) {
            let [in_first, _, in_both] = counts.entry(first).or_default();
            *in_first += 1;
            *in_both += u64::from(first == second);
            let [_, in_second, _] = counts.entry(second).or_default();
            *in_second += 1;
            differ += usize::from(first != second);
        }
        let dice = |(&label, &[first, second, both]): (&u32, &[u64; 3])| {
            (label, first, 2.0 * both as f64 / (first + second) as f64)
        };
        let labelled = counts.iter().filter(|(label, _)| **label != 0);
        Self {
            voxels: first.len().min(second.len()),
            differ,
            dice: labelled.map(dice).collect(),
        }
    }

    /// The share of voxels with the same label in both sets, in percent.
    pub fn same_percent(&self) -> f64 {
        100.0 * (self.voxels - self.differ) as f64 / self.voxels as f64
    }

    /// The least Dice of any label; 1 when neither set has a label but 0.
    pub fn least_dice(&self) -> f64 {
        let each = self.dice.iter().map(|&(_, _, dice)| dice);
        each.fold(1.0, f64::min)
    }
}

impl fmt::Display for Agreement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (voxels, differ, same) = (self.voxels, self.differ, self.same_percent());
        let (count, least) = (self.dice.len(), self.least_dice());
        let sum: f64 = self.dice.iter().map(|&(_, _, dice)| dice).sum();
        let mean = if count == 0 { 1.0 } else { sum / count as f64 };
        write!(
            f,
            "{differ} of {voxels} voxels differ ({same:.5}% the same); Dice of {count} \
             structures: least {least:.5}, mean {mean:.6}"
        )
    }
}

#[cfg(test)]
mod tests {
    use std::panic::catch_unwind;

    use volsa_models::{Card, CardId, CardKind, Device};

    use super::{Agreement, cards_of, once_each, quickest, watched};

    #[test]
    fn the_watcher_of_a_card_ends_with_the_work_even_when_the_work_panics() {
        // No computer has this card: nothing is read of it, and nothing was held.
        let nowhere = Device::Card(CardId(u64::MAX));
        assert_eq!(watched(nowhere, || 7), (7, Some(0)));
        assert_eq!(watched(Device::Processor, || 7), (7, None));
        let panicked = catch_unwind(|| watched(nowhere, || panic!("the work fails")));
        assert!(
            panicked.is_err(),
            "the panic is passed on once the watcher has ended"
        );
    }

    #[test]
    fn a_card_the_system_lists_twice_is_one_card_and_the_quickest_device_is_never_integrated() {
        let card = |index, id, name: &str, kind| Card {
            id: CardId(id),
            index,
            name: name.to_owned(),
            kind,
            own_bytes: 0,
        };
        // The graphics are listed three times: again as the same card to the system, and
        // again under the same name as another.
        let listed = vec![
            card(0, 100, "made-up graphics", CardKind::Integrated),
            card(1, 128, "a made-up card", CardKind::Discrete),
            card(
                2,
                100,
                "made-up graphics, listed again",
                CardKind::Integrated,
            ),
            card(3, 160, "drawn by the processor", CardKind::Software),
            card(4, 144, "made-up graphics", CardKind::Integrated),
            card(5, 96, "other made-up graphics", CardKind::Integrated),
        ];
        let places = |kind| -> Vec<u32> {
            let found = once_each(listed.clone(), kind);
            found.iter().map(|card| card.index).collect()
        };
        assert_eq!(places(CardKind::Integrated), [0, 5]);
        assert_eq!(places(CardKind::Discrete), [1]);
        assert_eq!(places(CardKind::Software), [3]);
        // On this computer, whatever it has: the processor, or a card that is discrete.
        match quickest() {
            (Device::Processor, name) => assert_eq!(name, "the processor"),
            (Device::Card(id), name) => {
                let discrete = cards_of(CardKind::Discrete);
                let is_one = |card: &Card| card.id == id && card.name == name;
                assert!(discrete.iter().any(is_one), "{name} is not a discrete card");
            }
        }
    }

    #[test]
    fn agreement_counts_the_voxels_that_differ_and_each_labels_dice() {
        // Made-up labels: 4 keeps two of its three voxels, 7 is found one voxel too large,
        // and 9 is only in the second set.
        let first = [0, 4, 4, 4, 7, 7, 0, 0];
        let second = [0, 4, 4, 7, 7, 7, 9, 0];
        let agreed = Agreement::of(&first, &second);
        assert_eq!((agreed.voxels, agreed.differ), (8, 2));
        let dice = vec![(4, 3, 0.8), (7, 2, 0.8), (9, 0, 0.0)];
        assert_eq!(agreed.dice, dice);
        assert_eq!(agreed.same_percent(), 75.0);
        assert_eq!(agreed.least_dice(), 0.0);
        let words = "2 of 8 voxels differ (75.00000% the same); Dice of 3 structures: least \
                     0.00000, mean 0.533333";
        assert_eq!(agreed.to_string(), words);
        // The same set twice agrees everywhere, and nothing but nought has no Dice to fall.
        let same = Agreement::of(&first, &first);
        assert_eq!((same.differ, same.least_dice()), (0, 1.0));
        assert_eq!(Agreement::of(&[0, 0], &[0, 0]).least_dice(), 1.0);
    }
}
