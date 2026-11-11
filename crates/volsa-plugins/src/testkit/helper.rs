//! A stand-in for the plugin helper: [`serve`] on a thread of this process over a pair of
//! pipes, with a network that answers two channels, a half and the value given, so that a
//! voxel is labelled 1 where the value it was given is above a half.

use std::io::{self, PipeReader, PipeWriter, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use volsa_models::testkit::Canned;
use volsa_models::{Network, Numbers, RunError, RuntimeError, Stop, Tensor};

use crate::helper::{Client, OpenJob, Opener, Where, serve};
use crate::{HelperPipes, HelperStarter};

/// A stand-in for the runtime whose network labels what is above a half.
#[derive(Debug, Clone, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the switches of a stand-in for tests"
)]
pub struct Threshold {
    /// Set when the helper is stopped; a slow network then answers "stopped".
    pub stopped: Arc<AtomicBool>,
    /// Whether each run waits a second, looking at `stopped`, before it answers.
    pub slow: bool,
    /// How many blocks have been run.
    pub runs: Arc<Mutex<usize>>,
    /// Whether the network will not open anywhere.
    pub refuse: bool,
    /// Whether the network will not open on a card, for want of room.
    pub no_room_on_cards: bool,
    /// Whether the helper dies as it runs a block.
    pub dies: bool,
}

impl Opener for Threshold {
    fn load(&self, _runtime: &Path) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn open(&self, job: &OpenJob, _stop: &Stop) -> Result<Box<dyn Network>, RunError> {
        if self.refuse {
            return Err(RunError::Failed("refused".to_owned()));
        }
        if self.no_room_on_cards && matches!(job.device, Where::Card { .. }) {
            return Err(RunError::NoRoom);
        }
        let (stopped, slow, runs) = (Arc::clone(&self.stopped), self.slow, Arc::clone(&self.runs));
        let dies = self.dies;
        Ok(Box::new(Canned::new(move |inputs: &[Tensor]| {
            *runs.lock().expect("runs") += 1;
            assert!(!dies, "the helper dies, as one might");
            for _ in 0..20 {
                if stopped.load(Ordering::SeqCst) {
                    return Err(RunError::Stopped);
                }
                if !slow {
                    break;
                }
                thread::sleep(Duration::from_millis(50));
            }
            let input = inputs.first().ok_or(RunError::Stopped)?;
            let Numbers::F32(values) = &input.numbers else {
                return Err(RunError::Failed("floats were expected".to_owned()));
            };
            let mut shape = input.shape.clone();
            if let Some(channels) = shape.get_mut(1) {
                *channels = 2;
            }
            let mut numbers = vec![0.5; values.len()];
            numbers.extend_from_slice(values);
            Tensor::f32(shape, numbers)
                .map(|tensor| vec![tensor])
                .ok_or(RunError::Stopped)
        })))
    }

    fn card_in_use(&self, place: &Where) -> Option<u64> {
        matches!(place, Where::Card { .. }).then_some(4 << 20)
    }
}

/// Starts in-process helpers with an opener, and remembers the caps they were given.
#[derive(Debug, Clone, Default)]
pub struct InProcessStarter {
    /// The opener every helper serves with.
    pub opener: Threshold,
    /// The cap each helper was started with, in order.
    pub caps: Arc<Mutex<Vec<Option<u64>>>>,
    /// What each helper says it held at most.
    pub peak: u64,
}

/// One in-process helper.
struct InProcess {
    pipes: Option<(PipeReader, PipeWriter)>,
    stopped: Arc<AtomicBool>,
    peak: u64,
}

impl HelperPipes for InProcess {
    fn client(&mut self) -> Option<Client<Box<dyn Read + Send>, Box<dyn Write + Send>>> {
        let (read, write) = self.pipes.take()?;
        Some(Client::new(Box::new(read), Box::new(write)))
    }

    fn stop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    fn peak_memory(&self) -> Option<u64> {
        Some(self.peak)
    }
}

impl HelperStarter for InProcessStarter {
    fn start(&self, cap: Option<u64>) -> io::Result<Box<dyn HelperPipes>> {
        self.caps.lock().expect("caps").push(cap);
        let (mut from_volsa, to_helper) = io::pipe()?;
        let (from_helper, mut to_volsa) = io::pipe()?;
        // Each helper has a stop of its own, as each process would.
        let stopped = Arc::new(AtomicBool::new(false));
        let opener = Threshold {
            stopped: Arc::clone(&stopped),
            ..self.opener.clone()
        };
        thread::spawn(move || {
            let _ = serve(&mut from_volsa, &mut to_volsa, &opener);
        });
        Ok(Box::new(InProcess {
            pipes: Some((from_helper, to_helper)),
            stopped,
            peak: self.peak,
        }))
    }
}
