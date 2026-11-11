//! The helper's real opener: ONNX Runtime, loaded from the folder Volsa names, with telemetry
//! off before anything else as [`Runtime::load`] makes it, and this computer's cards. And the
//! helper's whole life: one job served over standard input and output.

use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::sync::{Mutex, PoisonError};

use volsa_models::{CardId, Device, Network, RunError, Runtime, RuntimeError, Stop, in_use};

use super::{OpenJob, Opener, Where, serve};

/// For the log: a network asked for before any runtime was loaded.
const NOT_LOADED: &str = "no runtime has been loaded";

/// ONNX Runtime and this computer's cards.
#[derive(Debug, Default)]
pub struct RuntimeOpener {
    runtime: Mutex<Option<Runtime>>,
}

/// The device a place names.
fn device_of(place: &Where) -> Device {
    match place {
        Where::Processor => Device::Processor,
        Where::Card { id, .. } => Device::Card(CardId(*id)),
    }
}

impl Opener for RuntimeOpener {
    fn load(&self, runtime: &Path) -> Result<(), RuntimeError> {
        let mut held = self.runtime.lock().unwrap_or_else(PoisonError::into_inner);
        if held.is_none() {
            *held = Some(Runtime::load(runtime)?);
        }
        Ok(())
    }

    fn open(&self, job: &OpenJob, stop: &Stop) -> Result<Box<dyn Network>, RunError> {
        let held = self.runtime.lock().unwrap_or_else(PoisonError::into_inner);
        let runtime = held
            .clone()
            .ok_or_else(|| RunError::Failed(NOT_LOADED.to_owned()))?;
        drop(held);
        let device = device_of(&job.device);
        let output = job.output.as_deref();
        runtime.open_named(&job.network, device, &job.free, output, stop)
    }

    fn card_in_use(&self, place: &Where) -> Option<u64> {
        match device_of(place) {
            Device::Card(id) => in_use(id),
            Device::Processor => None,
        }
    }
}

/// The helper's whole life: one job served over standard input and output with the runtime,
/// then the end. It succeeds when the job was served to its end, whatever its outcome.
pub fn serve_stdio() -> ExitCode {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    match serve(&mut input, &mut output, &RuntimeOpener::default()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
