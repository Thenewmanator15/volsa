//! Knowing that the graphics device has stopped answering, and going on without it.
//!
//! A driver reset takes the device with it. `wgpu` then takes what the program asks of the
//! dead device for the program's own mistakes and, unless it is told otherwise, stops the
//! program with a panic. Neither `eframe` nor `egui-wgpu` tells it otherwise, and neither
//! can start a new device for a window that has lost its own. A [`DeviceWatch`] on the
//! window's device does what they do not: the program goes on, and learns that the device
//! is gone.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui_wgpu::wgpu;

/// Knows whether a graphics device has stopped answering, and keeps the device from stopping
/// the program once it has.
///
/// While the device answers, an error it gives is what it always was: `wgpu` stops the
/// program where the mistake was made, so that a bug is loud and a test fails where it did.
/// Once the device is lost, what is asked of it goes unanswered and is dropped without a
/// sound: all of it follows from the loss.
#[derive(Clone)]
pub struct DeviceWatch {
    /// Set once, when the device says it will answer no more.
    lost: Arc<AtomicBool>,
}

impl DeviceWatch {
    /// Start watching `device`. When it is lost, `wake` is called once, on whichever thread
    /// finds out, so that whatever waits on this watch can look. A device has room for one
    /// watch: watching it again takes it over from the first.
    pub fn on(device: &wgpu::Device, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let lost = Arc::new(AtomicBool::new(false));
        let on_loss = Arc::clone(&lost);
        device.set_device_lost_callback(move |_reason, _message| {
            on_loss.store(true, Ordering::SeqCst);
            wake();
        });
        let on_error = Arc::clone(&lost);
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
            answer(&on_error, &error);
        }));
        Self { lost }
    }

    /// Whether the device has stopped answering. It stays so: a lost device is not given
    /// back, and a window that has lost its own cannot be given another.
    pub fn is_lost(&self) -> bool {
        self.lost.load(Ordering::SeqCst)
    }
}

/// What is done with an error the device gave that no error scope took: nothing once the
/// device is lost; before that, what `wgpu` does by itself, which is to stop the program
/// with the error as the reason.
#[allow(
    clippy::panic,
    reason = "wgpu cannot be given its own answer back once another is set, and a mistake \
              of the program's must stay as loud as it was, so the same panic is made here"
)]
fn answer(lost: &AtomicBool, error: &wgpu::Error) {
    if lost.load(Ordering::SeqCst) {
        return;
    }
    panic!("wgpu error: {error}\n");
}

/// Run `work`, giving what it made, or `None` if it panicked.
pub(crate) fn held<T>(work: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(work)).ok()
}

/// Poll the device, giving how it stands, or `None` if it will not say. `wgpu` treats a
/// poll of a device that has been lost as a fatal mistake of the program and panics; here
/// that is a device that will not say.
pub(crate) fn poll(device: &wgpu::Device, how: wgpu::PollType) -> Option<wgpu::PollStatus> {
    held(|| device.poll(how)).and_then(Result::ok)
}

#[cfg(test)]
mod tests;
