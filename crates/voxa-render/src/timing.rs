//! How long the graphics device takes over a piece of work. A device does its work some
//! time after it is handed over and does not say how long it took, so the only way to know
//! is to hand it one piece when it has nothing else in hand, and wait.

use std::time::{Duration, Instant};

use egui_wgpu::wgpu;

/// Hand the device a piece of work and, if it had nothing else in hand, wait until it has
/// done it and say how long that took. With other work still in hand the piece is handed
/// over all the same, without waiting, and `None` is given: timed then, it would be charged
/// with the other work too.
pub fn timed_submit(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    work: wgpu::CommandBuffer,
) -> Option<Duration> {
    let idle = device
        .poll(wgpu::PollType::Poll)
        .is_ok_and(|status| status.is_queue_empty());
    let hand_over = || {
        queue.submit([work]);
    };
    let wait = || device.poll(wgpu::PollType::wait_indefinitely()).is_ok();
    timed_if(idle, hand_over, wait)
}

/// Hand a piece of work over, and time it only if the device was `idle`: then it is waited
/// for, and how long handing over and waiting took is given, unless the wait failed.
fn timed_if(idle: bool, hand_over: impl FnOnce(), wait: impl FnOnce() -> bool) -> Option<Duration> {
    let started = Instant::now();
    hand_over();
    (idle && wait()).then(|| started.elapsed())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn work_is_always_handed_over_and_waited_for_only_when_the_device_was_idle() {
        let (handed, waited) = (Cell::new(0), Cell::new(0));
        let hand_over = || handed.set(handed.get() + 1);
        let wait = || {
            waited.set(waited.get() + 1);
            true
        };
        assert_eq!(timed_if(false, hand_over, wait), None, "busy: not timed");
        assert_eq!((handed.get(), waited.get()), (1, 0), "and not waited for");
        assert!(timed_if(true, hand_over, wait).is_some(), "idle: timed");
        assert_eq!((handed.get(), waited.get()), (2, 1));
        let failed = || false;
        assert_eq!(timed_if(true, hand_over, failed), None, "a wait that fails");
        assert_eq!(handed.get(), 3, "the work was handed over all the same");
    }
}
