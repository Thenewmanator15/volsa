//! A network as the rest of Voxa sees it: tensors in, tensors out. What runs it (the runtime
//! on the processor or on a card, or a stand-in in a test) is behind the [`Network`] trait.
//!
//! The rules of a run that can be checked with no runtime are here too: how the runtime's
//! words are sorted into what went wrong, and when a card's outputs are fetched one by one.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::tensor::Tensor;

#[cfg(test)]
mod tests;

/// Windows' code for "not enough memory", as a runtime's words carry it.
const NO_MEMORY: &str = "8007000E";
/// Windows' codes for a graphics device that was removed, that hung and that was reset.
const DEVICE_GONE: [&str; 3] = ["887A0005", "887A0006", "887A0007"];
/// The sides of a network's input that are free until it is opened, by the names every
/// network of a model folder gives them.
pub(crate) const SIDES: [&str; 3] = ["x", "y", "z"];

/// Why a network gave no answer. Each sentence is fit to be shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    /// The device had not the memory the network needs.
    #[error("There was not enough memory to run the network.")]
    NoRoom,
    /// The graphics card was taken away, hung or was reset while the network was on it.
    #[error("The graphics card stopped answering while the network was on it.")]
    DeviceLost,
    /// A [`Stop`] was set.
    #[error("The run was stopped before the network had finished.")]
    Stopped,
    /// Anything else. It carries what went wrong in the runtime's own words, which are for
    /// the log only: they are not part of the sentence, and may name a file.
    #[error("The network could not be run.")]
    Failed(String),
}

impl RunError {
    /// Sorts what a runtime said into what went wrong. Windows' own codes are in its words.
    pub(crate) fn from_words(words: &str) -> Self {
        let capitals = words.to_ascii_uppercase();
        if capitals.contains(NO_MEMORY) {
            RunError::NoRoom
        } else if DEVICE_GONE.iter().any(|code| capitals.contains(code)) {
            RunError::DeviceLost
        } else {
            RunError::Failed(words.to_owned())
        }
    }
}

/// Something that turns tensors into tensors: a network opened on a device, or a stand-in.
pub trait Network: Send {
    /// Runs the network once. The inputs are in the network's own order, and so is the
    /// answer.
    ///
    /// # Errors
    ///
    /// [`RunError`] says why there is no answer. The network may be run again afterwards.
    fn run(&mut self, inputs: Vec<Tensor>) -> Result<Vec<Tensor>, RunError>;
}

/// A way to ask for a run to end. Every copy is the same switch: set through one, it is seen
/// through all, from any thread. It is never unset.
#[derive(Debug, Clone, Default)]
pub struct Stop(Arc<AtomicBool>);

impl Stop {
    /// A switch that is not set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Asks for whatever watches this switch to end.
    pub fn stop(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether an end has been asked for.
    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Where a network runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Device {
    /// The processor, in ordinary memory.
    Processor,
    /// The graphics card at this place in the system's list now: a [`Card`](crate::Card)'s
    /// `index`. The list can change, so the place is looked up when a network is opened and
    /// not remembered.
    Card(u32),
}

/// How many bytes an output holds, its numbers `width` bytes each. In `shape`, as a network
/// file gives it, a side left free is below zero and goes by a name in `names`. Such a side
/// is known when its name is one of the three the `sizes` are told by; `None` when one is
/// not, or the count is too large.
pub(crate) fn bytes_of(
    width: u64,
    shape: &[i64],
    names: &[String],
    sizes: [u64; 3],
) -> Option<u64> {
    let told = |place: usize| {
        let name = names.get(place)?;
        let which = SIDES.iter().position(|side| side == name)?;
        sizes.get(which).copied()
    };
    let mut bytes = width;
    for (place, &side) in shape.iter().enumerate() {
        let side = u64::try_from(side).ok().or_else(|| told(place))?;
        bytes = bytes.checked_mul(side)?;
    }
    Some(bytes)
}

/// Whether each output is fetched by a run of its own: on a card, when there is more than
/// one output and together they hold more than `most` bytes, or may for all that is known.
pub(crate) fn each_alone(device: Device, outputs: usize, fetched: Option<u64>, most: u64) -> bool {
    matches!(device, Device::Card(_)) && outputs > 1 && fetched.is_none_or(|bytes| bytes > most)
}
