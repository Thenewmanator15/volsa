//! A network the runtime has opened on one device, and a run of it: the inputs handed over
//! without a copy, the outputs fetched together or one by one, and a run told to end when its
//! stop is set.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;

use ort::session::SessionOutputs;
use ort::session::{OutputSelector, RunOptions, Session, SessionInputValue, SessionInputs};
use ort::value::{DynValue, TensorElementType};

use super::{LOOK_EVERY, failed, guarded, said};
use crate::network::{Network, RunError, Stop};
use crate::tensor::{Numbers, Tensor, copied};

/// A tensor in the runtime's hands, ready to be an input.
type Feed = SessionInputValue<'static>;

/// A network file opened by the runtime on one device.
pub(super) struct Opened {
    pub(super) session: Session,
    /// How many inputs the network takes.
    pub(super) inputs: usize,
    /// The names of its outputs, in its own order.
    pub(super) outputs: Vec<String>,
    /// Whether each output is fetched by a run of its own.
    pub(super) alone: bool,
    pub(super) stop: Stop,
}

impl Network for Opened {
    fn run(&mut self, inputs: Vec<Tensor>) -> Result<Vec<Tensor>, RunError> {
        if self.stop.is_set() {
            return Err(RunError::Stopped);
        }
        match guarded(|| self.answer(inputs)) {
            // A run the runtime ended because it was told to fails in the runtime's own
            // words. What happened is that it was stopped.
            Err(_) if self.stop.is_set() => Err(RunError::Stopped),
            outcome => outcome,
        }
    }
}

impl Opened {
    fn answer(&mut self, inputs: Vec<Tensor>) -> Result<Vec<Tensor>, RunError> {
        if inputs.len() != self.inputs {
            let (takes, given) = (self.inputs, inputs.len());
            let why = format!("inputs the network takes: {takes}; inputs given: {given}");
            return Err(RunError::Failed(why));
        }
        let feeds: Vec<Feed> = inputs.into_iter().map(feed).collect::<Result<_, _>>()?;
        if self.alone {
            return self.one_by_one(&feeds);
        }
        // One run that fetches every output, while another thread looks at the stop.
        let options = said(RunOptions::new())?;
        let got = said(watched(&mut self.session, &feeds, &options, &self.stop))?;
        let names = self.outputs.iter();
        names.map(|name| tensor(got.get(name))).collect()
    }

    /// A run for each output that asks for that output alone. `ort` lets no other thread
    /// reach such a run, so the stop is looked at between them and not during one.
    fn one_by_one(&mut self, feeds: &[Feed]) -> Result<Vec<Tensor>, RunError> {
        let mut answer = Vec::with_capacity(self.outputs.len());
        for name in &self.outputs {
            if self.stop.is_set() {
                return Err(RunError::Stopped);
            }
            let only = OutputSelector::no_default().with(name.as_str());
            let only = said(RunOptions::new())?.with_outputs(only);
            let feeds = SessionInputs::ValueSlice::<0>(feeds);
            let got = said(self.session.run_with_options(feeds, &only))?;
            answer.push(tensor(got.get(name))?);
        }
        Ok(answer)
    }
}

/// One run under `options`, while another thread looks at `stop` and tells the runtime to
/// end the run once it is set.
fn watched<'r>(
    session: &'r mut Session,
    feeds: &[Feed],
    options: &'r RunOptions,
    stop: &Stop,
) -> ort::Result<SessionOutputs<'r>> {
    thread::scope(move |scope| {
        let (done, wait) = mpsc::channel::<()>();
        let watch = move || {
            while wait.recv_timeout(LOOK_EVERY) == Err(RecvTimeoutError::Timeout) {
                if stop.is_set() {
                    // If the runtime will not be told, the run ends in its own time.
                    let _ = options.terminate();
                    return;
                }
            }
        };
        // Without a watcher the run still goes on; it only cannot be ended early.
        let _ = thread::Builder::new().spawn_scoped(scope, watch);
        let outcome = session.run_with_options(SessionInputs::ValueSlice::<0>(feeds), options);
        drop(done);
        outcome
    })
}

/// Hands a tensor to the runtime. Its numbers are not copied.
fn feed(tensor: Tensor) -> Result<Feed, RunError> {
    if !tensor.is_whole() {
        return Err(failed("an input is not as long as its shape says"));
    }
    let Tensor { shape, numbers } = tensor;
    said(match numbers {
        Numbers::F32(numbers) => ort::value::Tensor::from_array((shape, numbers)).map(Feed::from),
        Numbers::U8(numbers) => ort::value::Tensor::from_array((shape, numbers)).map(Feed::from),
        Numbers::I32(numbers) => ort::value::Tensor::from_array((shape, numbers)).map(Feed::from),
        Numbers::I64(numbers) => ort::value::Tensor::from_array((shape, numbers)).map(Feed::from),
    })
}

/// Copies an output out of the runtime's own memory.
fn tensor(value: Option<&DynValue>) -> Result<Tensor, RunError> {
    let value = value.ok_or_else(|| failed("the network left out one of its outputs"))?;
    let (shape, numbers) = match value.dtype().tensor_type() {
        Some(TensorElementType::Float32) => {
            let (shape, numbers) = said(value.try_extract_tensor::<f32>())?;
            (shape, Numbers::F32(copied(numbers)?))
        }
        Some(TensorElementType::Uint8) => {
            let (shape, numbers) = said(value.try_extract_tensor::<u8>())?;
            (shape, Numbers::U8(copied(numbers)?))
        }
        Some(TensorElementType::Int32) => {
            let (shape, numbers) = said(value.try_extract_tensor::<i32>())?;
            (shape, Numbers::I32(copied(numbers)?))
        }
        Some(TensorElementType::Int64) => {
            let (shape, numbers) = said(value.try_extract_tensor::<i64>())?;
            (shape, Numbers::I64(copied(numbers)?))
        }
        _ => return Err(failed("an output of a kind Voxa does not read")),
    };
    let shape: Result<_, _> = shape.iter().map(|&side| usize::try_from(side)).collect();
    let shape = shape.map_err(|_| failed("an output with a side below zero"))?;
    Ok(Tensor { shape, numbers })
}
