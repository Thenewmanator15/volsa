//! What a run of labelling is done with: the runtime that opens the networks, and what the
//! computer has for them. The workers use the runtime and this computer's own; a test gives
//! stand-ins, so that a run can be followed with no runtime, no network file and no card.

use std::path::Path;
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use voxa_models::{
    Card, CardId, Device, FreeMemory, Model, Network, Role, RunError, Rung, Runtime, RuntimeError,
    Stop,
};
use voxa_segment::Networks;

/// Seconds in a day.
const A_DAY: u64 = 24 * 60 * 60;
/// For the log: a network asked for before any runtime was loaded.
const NOT_LOADED: &str = "no runtime has been loaded";

/// What a run of labelling is done with: the runtime that opens the networks, and what the
/// computer has for them, each read at the moment it is asked for.
/// [`Workers::new`](crate::Workers::new) takes one, and with none uses the runtime and this
/// computer's own. A test gives stand-ins.
pub trait Labeller: Send + Sync {
    /// Has the runtime in `folder` ready to open networks. The runtime is loaded once in the
    /// program's life: once one is had, it is had again whatever folder is named.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] says why the runtime could not be had from that folder.
    fn load_runtime(&self, folder: &Path) -> Result<(), RuntimeError>;

    /// Opens the network of `role` in `model` on `device`, for a grid of these `sizes`. A run
    /// in progress is ended when `stop` is set, where that can be done.
    ///
    /// # Errors
    ///
    /// [`RunError`] says why the network could not be opened.
    fn open(
        &self,
        model: &Model,
        role: Role,
        device: Device,
        sizes: [usize; 3],
        stop: &Stop,
    ) -> Result<Box<dyn Network>, RunError>;

    /// The graphics cards, as the system lists them now.
    fn cards(&self) -> Vec<Card>;

    /// How many bytes the system lets this program use on a card now; `None` where that
    /// cannot be read.
    fn budget(&self, card: CardId) -> Option<u64>;

    /// How much more ordinary memory this program can have now.
    fn free_memory(&self) -> FreeMemory;

    /// Today's date, as it is written for a person.
    fn today(&self) -> String;
}

/// The networks of one rung: the scores network on the rung's `scores_on`, every other on its
/// device.
pub(super) struct OnRung<'a> {
    pub(super) labeller: &'a dyn Labeller,
    pub(super) model: &'a Model,
    pub(super) rung: Rung,
    pub(super) stop: &'a Stop,
}

impl Networks for OnRung<'_> {
    fn open(&mut self, role: Role, sizes: [usize; 3]) -> Result<Box<dyn Network>, RunError> {
        let device = match role {
            Role::Scores => self.rung.scores_on,
            _ => self.rung.device,
        };
        self.labeller
            .open(self.model, role, device, sizes, self.stop)
    }
}

/// This computer, and the runtime once it has been loaded.
#[derive(Default)]
pub(in crate::worker) struct ThisComputer {
    runtime: Mutex<Option<Runtime>>,
}

impl Labeller for ThisComputer {
    fn load_runtime(&self, folder: &Path) -> Result<(), RuntimeError> {
        let mut held = self.runtime.lock().unwrap_or_else(PoisonError::into_inner);
        if held.is_none() {
            *held = Some(Runtime::load(folder)?);
        }
        Ok(())
    }

    fn open(
        &self,
        model: &Model,
        role: Role,
        device: Device,
        sizes: [usize; 3],
        stop: &Stop,
    ) -> Result<Box<dyn Network>, RunError> {
        let held = self.runtime.lock().unwrap_or_else(PoisonError::into_inner);
        let runtime = held.clone();
        drop(held);
        let runtime = runtime.ok_or_else(|| RunError::Failed(NOT_LOADED.to_owned()))?;
        runtime.open(model.file(role), device, sizes, stop)
    }

    fn cards(&self) -> Vec<Card> {
        voxa_models::cards()
    }

    fn budget(&self, card: CardId) -> Option<u64> {
        voxa_models::budget(card)
    }

    fn free_memory(&self) -> FreeMemory {
        voxa_models::free_memory()
    }

    fn today(&self) -> String {
        let since = SystemTime::now().duration_since(UNIX_EPOCH);
        date_of(since.unwrap_or_default().as_secs() / A_DAY)
    }
}

/// The date so many days after the first of January 1970, by the calendar in use now, as
/// year, month and day: `YYYY-MM-DD`. The day is the one at Greenwich.
pub(super) fn date_of(days: u64) -> String {
    // Counted from the first of March of the year 0: a leap day then ends its year. An era
    // is 400 years, which hold the same number of days each.
    let days = days.saturating_add(719_468);
    let (era, day_of_era) = (days / 146_097, days % 146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = era * 400 + year_of_era + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
