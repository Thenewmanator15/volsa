//! Whose turn it is to have networks open, and the threads that labelling's long work goes on.
//!
//! A run takes minutes and a look seconds. Each goes on a thread of its own, made for it and
//! ended with it, and not on the workers' shared pool: work that long on the pool could hold
//! up the decoding of slices, or be taken up by a thread of the pool that waits inside other
//! work, which would then wait for it. A run waits on its own thread for its turn. A run that
//! is stopped while it waits gives up at once, and so lets its volume go; one that is stopped
//! as it is given the turn gives it back before it does anything.

use std::sync::PoisonError;

use volsa_models::Stop;

use super::Kit;
use crate::worker::{lock, shielded};

/// The name of the threads that labelling's long work goes on.
pub(super) const THREAD: &str = "volsa-labelling";

/// A run's turn to have networks open. It is given back when it is dropped, however the run
/// ended, a panic included.
pub(super) struct Turn<'a>(&'a Kit);

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        *lock(&self.0.busy) = false;
        self.0.freed.notify_all();
    }
}

impl Kit {
    /// The turn, once no other run has it. `None`, at once, when `stop` is set first.
    pub(super) fn turn(&self, stop: &Stop) -> Option<Turn<'_>> {
        let mut busy = lock(&self.busy);
        while *busy && !stop.is_set() {
            busy = self
                .freed
                .wait(busy)
                .unwrap_or_else(PoisonError::into_inner);
        }
        if stop.is_set() {
            return None;
        }
        *busy = true;
        Some(Turn(self))
    }

    /// Wake the runs that wait for their turn, so that one that has been stopped gives up. The
    /// lock is taken first, so that a run between looking at its stop and waiting is not missed.
    pub(super) fn wake(&self) {
        let _busy = lock(&self.busy);
        self.freed.notify_all();
    }
}

/// Do `work` on a thread of its own, so that a panic inside it ends that work and nothing
/// else. False when the system would not make a thread: the work is not done.
pub(super) fn on_its_own(work: impl FnOnce() + Send + 'static) -> bool {
    let named = std::thread::Builder::new().name(THREAD.to_owned());
    named
        .spawn(move || {
            shielded(work);
        })
        .is_ok()
}
