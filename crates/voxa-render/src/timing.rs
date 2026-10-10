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
    let started = Instant::now();
    queue.submit([work]);
    if !idle {
        return None;
    }
    device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
    Some(started.elapsed())
}
