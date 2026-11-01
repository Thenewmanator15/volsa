//! ONNX Runtime behind [`Network`]: its library loaded from a folder the person names, a
//! network file opened on the processor or on a graphics card, and a run.
//!
//! Every call into the runtime is made here, and none of them is `unsafe`. A panic on the
//! way becomes [`RunError::Failed`], so that it cannot take down whatever asked for the run.

use std::panic::{self, AssertUnwindSafe};
use std::path::{self, Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use ort::environment::Environment;
use ort::session::SessionOutputs;
use ort::session::builder::SessionBuilder;
use ort::session::{OutputSelector, RunOptions, Session, SessionInputValue, SessionInputs};
use ort::value::{DynValue, Outlet, TensorElementType, ValueType};

use crate::cards::CardId;
#[cfg(windows)]
use crate::cards::place_now;
use crate::network::{Device, Network, RunError, SIDES, Stop, bytes_of, each_alone, not_loaded};
use crate::tensor::{Numbers, Tensor, copied};

/// The file that holds the runtime, by the name this system gives such a file.
pub(crate) const LIBRARY: &str = if cfg!(windows) {
    "onnxruntime.dll"
} else if cfg!(target_os = "macos") {
    "libonnxruntime.dylib"
} else {
    "libonnxruntime.so"
};
/// Most that one run fetches from a card, in bytes: 2 GiB.
const MOST_FETCHED: u64 = 1 << 31;
/// How often a run in progress looks at its [`Stop`].
const LOOK_EVERY: Duration = Duration::from_millis(50);

/// The library this program holds, once it holds one: a program can hold only one, for as
/// long as it lives.
static HELD: Mutex<Option<PathBuf>> = Mutex::new(None);

/// A tensor in the runtime's hands, ready to be an input.
type Feed = SessionInputValue<'static>;

/// Why the runtime could not be had from a folder. Each sentence is fit to be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RuntimeError {
    /// The folder does not hold the runtime's library.
    #[error("The runtime is not in the folder given.")]
    NotFound,
    /// The runtime there is older than the one this version of Volsa asks for.
    #[error("The runtime in that folder is older than this version of Volsa needs.")]
    WrongBuild,
    /// The library is there but would not load, or the program already holds the runtime of
    /// another folder.
    #[error("The runtime in that folder would not load.")]
    WouldNotLoad,
    /// The library would not load for want of a library it needs, or with one too old: most
    /// likely Microsoft's Visual C++ runtime, which ONNX Runtime needs and Volsa does not.
    #[error(
        "The runtime in that folder would not load, most likely because this computer lacks \
         the Microsoft Visual C++ 2015-2022 x64 redistributable, or has an old one. ONNX \
         Runtime needs it, and Microsoft gives it free."
    )]
    NeedsVisualCpp,
}

/// The runtime, loaded. Having one is what lets a network be opened.
#[derive(Debug, Clone)]
pub struct Runtime {
    _held: (),
}

impl Runtime {
    /// Loads the runtime's library from `folder`. Its telemetry is off before this returns,
    /// and so before any network can be opened.
    ///
    /// A program holds one runtime for as long as it lives. A second call for the same
    /// folder gives the runtime again and loads nothing. A call for another folder is
    /// refused with [`RuntimeError::WouldNotLoad`] until the program is started again. After
    /// a call that failed, the same folder or another may be tried.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] says what is wrong, in words fit to be shown.
    pub fn load(folder: &Path) -> Result<Self, RuntimeError> {
        let library = folder.join(LIBRARY);
        if !library.is_file() {
            return Err(RuntimeError::NotFound);
        }
        // One spelling of the file however the folder was written, to know it again.
        let (Ok(known_as), Ok(library)) = (library.canonicalize(), path::absolute(&library)) else {
            return Err(RuntimeError::NotFound);
        };
        let mut held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
        match held.as_ref() {
            Some(first) if *first == known_as => {}
            Some(_) => return Err(RuntimeError::WouldNotLoad),
            None => {
                start(&library)?;
                *held = Some(known_as);
            }
        }
        // The settings take hold when the runtime's environment is made. It is made here,
        // while no network can yet be opened, so telemetry is off before the first one is.
        match caught(Environment::current) {
            Ok(Ok(_)) => Ok(Self { _held: () }),
            _ => Err(RuntimeError::WouldNotLoad),
        }
    }

    /// Opens the network in `file` on `device`, for an input whose free sides have these
    /// `sizes`. The network is told the sizes now, by the names `x`, `y` and `z`, and can
    /// then be run on no other size. On a card it runs through DirectML and follows its
    /// rules, and where the card stands in the system's list is looked up here, now. On the
    /// processor the runtime keeps no memory back for later and plans none ahead.
    ///
    /// `stop` is kept. It is looked at before every run, and a run in progress is told to
    /// end when it is set. The runtime ends one where it can, which is between two steps of a
    /// network on the processor. A card finishes what it was handed, and the run answers.
    ///
    /// One run fetches every output. When together they would pass 2 GiB on a card, each is
    /// fetched by a run of its own that asks for it alone, so the network is worked out once
    /// for each; those runs look at `stop` only between them.
    ///
    /// # Errors
    ///
    /// [`RunError::Stopped`] when `stop` is already set, [`RunError::NoRoom`] when the device
    /// has not the memory, [`RunError::DeviceLost`] when the card went or is no longer in
    /// the system's list, and [`RunError::Failed`] for anything else: no such file, a file
    /// that is no network, or a system on which a card cannot be used.
    pub fn open(
        &self,
        file: &Path,
        device: Device,
        sizes: [usize; 3],
        stop: &Stop,
    ) -> Result<Box<dyn Network>, RunError> {
        self.open_with(file, device, sizes, stop, MOST_FETCHED)
    }

    /// [`Runtime::open`] with the most that one run may fetch from a card given, so that a
    /// test can have each output fetched alone without a network that large.
    #[expect(
        clippy::unused_self,
        reason = "having a runtime is what shows it is loaded"
    )]
    pub(crate) fn open_with(
        &self,
        file: &Path,
        device: Device,
        sizes: [usize; 3],
        stop: &Stop,
        most: u64,
    ) -> Result<Box<dyn Network>, RunError> {
        if stop.is_set() {
            return Err(RunError::Stopped);
        }
        let session = guarded(|| session(file, device, sizes))?;
        let outputs = session.outputs().iter().map(Outlet::name);
        Ok(Box::new(Opened {
            alone: each_alone(device, outputs.len(), fetched(&session, sizes), most),
            inputs: session.inputs().len(),
            outputs: outputs.map(str::to_owned).collect(),
            session,
            stop: stop.clone(),
        }))
    }
}

/// Loads the library and gives the runtime its settings: telemetry off.
fn start(library: &Path) -> Result<(), RuntimeError> {
    let settings = match caught(|| ort::init_from(library)) {
        Ok(Ok(settings)) => settings,
        Ok(Err(why)) => return Err(not_loaded(&why)),
        Err(_) => return Err(RuntimeError::WouldNotLoad),
    };
    // Not taken would mean something else gave the runtime its settings first, and then
    // telemetry could not be known to be off.
    if settings.with_name("volsa").with_telemetry(false).commit() {
        Ok(())
    } else {
        Err(RuntimeError::WouldNotLoad)
    }
}

/// Does work that calls into the runtime. A panic there comes back as its words.
fn caught<T>(work: impl FnOnce() -> T) -> Result<T, String> {
    panic::catch_unwind(AssertUnwindSafe(work)).map_err(|panic| {
        let words = panic.downcast_ref::<&str>().map(ToString::to_string);
        let words = words.or_else(|| panic.downcast_ref::<String>().cloned());
        format!("the runtime panicked: {}", words.unwrap_or_default())
    })
}

/// Does work that calls into the runtime, a panic there becoming [`RunError::Failed`].
fn guarded<T>(work: impl FnOnce() -> Result<T, RunError>) -> Result<T, RunError> {
    caught(work).unwrap_or_else(|words| Err(RunError::Failed(words)))
}

/// What the runtime gave, or what went wrong sorted from the words it said.
fn said<T, R>(outcome: Result<T, ort::Error<R>>) -> Result<T, RunError> {
    outcome.map_err(|error| RunError::from_words(error.message()))
}

fn failed(why: &str) -> RunError {
    RunError::Failed(why.to_owned())
}

/// Makes the runtime's session for a network file: the sizes told by name and, on a card,
/// DirectML with its rules.
fn session(file: &Path, device: Device, sizes: [usize; 3]) -> Result<Session, RunError> {
    let mut builder = said(Session::builder())?;
    for (name, size) in SIDES.into_iter().zip(sizes) {
        let size = i64::try_from(size).map_err(|_| failed("a side of the input is too long"))?;
        builder = said(builder.with_dimension_override(name, size))?;
    }
    let mut builder = match device {
        Device::Card(card) => on_card(builder, card)?,
        Device::Processor => on_the_processor(builder)?,
    };
    said(builder.commit_from_file(file))
}

/// Puts a session on the processor with the runtime's arena and its memory pattern both
/// off. With them on it asks the system for several times what the network holds.
fn on_the_processor(builder: SessionBuilder) -> Result<SessionBuilder, RunError> {
    let plain = ort::ep::CPU::default().with_arena_allocator(false).build();
    let builder = said(builder.with_execution_providers([plain.error_on_failure()]))?;
    said(builder.with_memory_pattern(false))
}

/// Puts a session on a card, at the place the card has in the system's list now. DirectML
/// asks that a network's steps run one after another and that memory is not planned ahead.
#[cfg(windows)]
fn on_card(builder: SessionBuilder, card: CardId) -> Result<SessionBuilder, RunError> {
    // A card the system no longer lists is a card that went.
    let index = place_now(card).ok_or(RunError::DeviceLost)?;
    let index = i32::try_from(index).map_err(|_| failed("there is no card at that place"))?;
    let directml = ort::ep::DirectML::default().with_device_id(index).build();
    let builder = said(builder.with_execution_providers([directml.error_on_failure()]))?;
    let builder = said(builder.with_parallel_execution(false))?;
    said(builder.with_memory_pattern(false))
}

/// Where there is no DirectML a network cannot be put on a card.
#[cfg(not(windows))]
fn on_card(_builder: SessionBuilder, _card: CardId) -> Result<SessionBuilder, RunError> {
    Err(failed("a card can run a network only on Windows"))
}

/// How many bytes the outputs of a network hold together, for an input of these sizes.
/// `None` when that cannot be known before a run.
fn fetched(session: &Session, sizes: [usize; 3]) -> Option<u64> {
    let sizes = sizes.map(|side| side as u64);
    session.outputs().iter().try_fold(0_u64, |total, output| {
        let kind = output.dtype();
        let width = match kind.tensor_type()? {
            TensorElementType::Float32 => 4,
            TensorElementType::Uint8 => 1,
            _ => return None,
        };
        let ValueType::Tensor {
            dimension_symbols: names,
            ..
        } = kind
        else {
            return None;
        };
        total.checked_add(bytes_of(width, kind.tensor_shape()?, names, sizes)?)
    })
}

/// A network file opened by the runtime on one device.
struct Opened {
    session: Session,
    /// How many inputs the network takes.
    inputs: usize,
    /// The names of its outputs, in its own order.
    outputs: Vec<String>,
    /// Whether each output is fetched by a run of its own.
    alone: bool,
    stop: Stop,
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
        _ => return Err(failed("an output of a kind Volsa does not read")),
    };
    let shape: Result<_, _> = shape.iter().map(|&side| usize::try_from(side)).collect();
    let shape = shape.map_err(|_| failed("an output with a side below zero"))?;
    Ok(Tensor { shape, numbers })
}
