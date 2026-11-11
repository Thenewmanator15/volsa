//! What passes between Volsa and its plugin helper, `volsa.exe --plugin-helper`, over the
//! helper's input and output: the job, the blocks of the scan as 32-bit numbers, and the
//! labels the helper finishes from each answer, as 16-bit numbers. Scores never cross the pipe,
//! and nothing goes to disk.
//!
//! Each message is a frame: a byte for its kind, its length in eight bytes, and that many
//! bytes. The job and what the helper says of itself are JSON; numbers are little-endian.
//!
//! The helper checks the network file's sum before it opens it, so a file changed after it
//! was added is refused. A helper that ends without answering is a failure, never a wait.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use volsa_models::{Network, RunError, RuntimeError, Stop};

use crate::finish::FinishError;
use crate::{LabelOutput, Layout};

mod client;
pub(crate) mod frame;
mod runtime_opener;
mod serve;

pub use client::Client;
pub use runtime_opener::{RuntimeOpener, serve_stdio};
pub use serve::{ServeError, serve};

#[cfg(test)]
mod frame_tests;
#[cfg(test)]
mod tests;

/// Where a network runs, as the helper is told.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Where {
    /// The processor.
    Processor,
    /// A graphics card, by the system's identity for it and its name.
    Card {
        /// The card's identity, as [`volsa_models::CardId`] holds it.
        id: u64,
        /// The card's name.
        name: String,
    },
}

/// What the helper is asked to do: open one network and finish each block's answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenJob {
    /// The folder of the runtime.
    pub runtime: PathBuf,
    /// The network file, in Volsa's plugins folder.
    pub network: PathBuf,
    /// The network file's SHA-256, as the plugin's description lists it.
    pub sha256: String,
    /// Where the network runs.
    pub device: Where,
    /// The network's free sides, by name, with their sizes.
    pub free: Vec<(String, usize)>,
    /// The output to read, when the network has several.
    pub output: Option<String>,
    /// Where the input's one channel lies.
    pub layout: Layout,
    /// The size of every block.
    pub block: [usize; 3],
    /// How a block's answer is finished to labels.
    pub finish: LabelOutput,
}

/// Why the helper gave no answer, as it says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Failure {
    /// The device had not the memory.
    NoRoom,
    /// The graphics card stopped answering.
    DeviceLost,
    /// The run was stopped.
    Stopped,
    /// The network file is not the one the description lists.
    Changed,
    /// The answer could not be finished to labels.
    Answer(FinishError),
    /// Anything else, in the runtime's words, for the log only.
    Failed(String),
}

/// Why a run in the helper gave no answer. Each sentence is fit to be shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HelperError {
    /// The helper ended, or its pipes closed, before it answered.
    #[error("The plugin stopped without finishing. Volsa is unaffected.")]
    Ended,
    /// The network file changed since the plugin was added.
    #[error("This plugin's files have changed since it was added. Remove it and add it again.")]
    Changed,
    /// The network could not be opened or run.
    #[error(transparent)]
    Run(RunError),
    /// The network's answer could not be finished to labels.
    #[error(transparent)]
    Answer(FinishError),
}

impl From<Failure> for HelperError {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::NoRoom => HelperError::Run(RunError::NoRoom),
            Failure::DeviceLost => HelperError::Run(RunError::DeviceLost),
            Failure::Stopped => HelperError::Run(RunError::Stopped),
            Failure::Changed => HelperError::Changed,
            Failure::Answer(why) => HelperError::Answer(why),
            Failure::Failed(words) => HelperError::Run(RunError::Failed(words)),
        }
    }
}

impl From<RunError> for Failure {
    fn from(why: RunError) -> Self {
        match why {
            RunError::NoRoom => Failure::NoRoom,
            RunError::DeviceLost => Failure::DeviceLost,
            RunError::Stopped => Failure::Stopped,
            RunError::Failed(words) => Failure::Failed(words),
        }
    }
}

/// What the helper opens networks with: the runtime and this computer's cards, or a stand-in.
pub trait Opener {
    /// Has the runtime of `runtime` ready.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] when it could not be had.
    fn load(&self, runtime: &Path) -> Result<(), RuntimeError>;

    /// Opens the job's network on its device.
    ///
    /// # Errors
    ///
    /// [`RunError`] when it could not be opened.
    fn open(&self, job: &OpenJob, stop: &Stop) -> Result<Box<dyn Network>, RunError>;

    /// How many bytes of the card the helper holds now, if it runs on one and that can be read.
    fn card_in_use(&self, place: &Where) -> Option<u64>;
}
