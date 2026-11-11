//! A helper watched while Volsa works with it, for a run and for a measurement alike: ended at
//! once when the stop is set, on a card too, and ended when the work is done.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use volsa_models::Stop;

use crate::HelperStarter;
use crate::helper::{Client, HelperError};

/// Volsa's end of a helper's pipes.
pub(crate) type Pipes = Client<Box<dyn Read + Send>, Box<dyn Write + Send>>;

/// How often the watch looks at the stop.
const LOOK_EVERY: Duration = Duration::from_millis(50);

/// Does `work` with a helper started by `starter`, its ordinary memory capped at `cap` when one
/// is given, ending the helper at once when `stop` is set and when the work is done. Gives what
/// the work gave and the most ordinary memory the helper held, read before it is ended.
///
/// # Errors
///
/// [`HelperError::Ended`] when the helper could not be started.
pub(crate) fn on_helper<T>(
    starter: &dyn HelperStarter,
    cap: Option<u64>,
    stop: &Stop,
    work: impl FnOnce(&mut Pipes) -> T,
) -> Result<(T, Option<u64>), HelperError> {
    let pipes = starter.start(cap).map_err(|_| HelperError::Ended)?;
    let helper = Mutex::new(pipes);
    let held = || helper.lock().unwrap_or_else(PoisonError::into_inner);
    let mut client = held().client().ok_or(HelperError::Ended)?;
    let done = AtomicBool::new(false);
    let outcome = thread::scope(|scope| {
        scope.spawn(|| {
            while !done.load(Ordering::SeqCst) {
                if stop.is_set() {
                    held().stop();
                    return;
                }
                thread::sleep(LOOK_EVERY);
            }
        });
        let outcome = work(&mut client);
        done.store(true, Ordering::SeqCst);
        outcome
    });
    let mut helper = held();
    let peak = helper.peak_memory();
    helper.stop();
    Ok((outcome, peak))
}
