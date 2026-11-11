//! Watching a graphics device for a loss, on devices destroyed to make them so.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use egui_wgpu::wgpu;

use super::*;
use crate::gpu::Gpu;
use crate::slice_pipeline::RenderError;

/// A device with no window, or `None` with a printed notice on a machine with no adapter.
fn started() -> Option<Gpu> {
    match Gpu::start() {
        Ok(gpu) => Some(gpu),
        Err(RenderError::NoAdapter) => {
            eprintln!("skipped: no graphics adapter here, not even a software one");
            None
        }
        Err(other) => panic!("{other}"),
    }
}

/// A watch on the device, and how many times it has woken what waits on it.
fn watched(gpu: &Gpu) -> (DeviceWatch, Arc<AtomicU32>) {
    let woken = Arc::new(AtomicU32::new(0));
    let count = Arc::clone(&woken);
    let wake = move || {
        count.fetch_add(1, Ordering::SeqCst);
    };
    (DeviceWatch::on(&gpu.device, wake), woken)
}

/// Ask the device for a buffer that is to be used for nothing, which it refuses.
fn refused_a_buffer(gpu: &Gpu) {
    let _ = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("made-up"),
        size: 16,
        usage: wgpu::BufferUsages::empty(),
        mapped_at_creation: false,
    });
}

#[test]
fn work_that_panics_is_held_and_work_that_does_not_gives_what_it_made() {
    assert_eq!(held(|| 7), Some(7));
    let panicked: Option<u32> = held(|| panic!("made-up trouble"));
    assert_eq!(panicked, None);
}

#[test]
fn a_watch_hears_that_its_device_is_lost_and_wakes_what_waits_on_it_once() {
    let Some(gpu) = started() else { return };
    let (watch, woken) = watched(&gpu);
    assert!(!watch.is_lost());
    assert_eq!(woken.load(Ordering::SeqCst), 0);
    gpu.lose();
    assert!(watch.is_lost());
    assert_eq!(woken.load(Ordering::SeqCst), 1);
    gpu.lose();
    assert_eq!(woken.load(Ordering::SeqCst), 1, "once");
}

#[test]
fn an_error_from_a_device_that_answers_stops_the_program_where_it_happens() {
    let Some(gpu) = started() else { return };
    let (watch, woken) = watched(&gpu);
    let stopped = catch_unwind(AssertUnwindSafe(|| refused_a_buffer(&gpu)));
    let why = stopped.expect_err("the program was to stop at the refusal");
    let said = why.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(said.starts_with("wgpu error: "), "{said}");
    assert!(said.contains("usage"), "{said}");
    // It was a mistake of the program's, not a loss of the device's.
    assert!(!watch.is_lost());
    assert_eq!(woken.load(Ordering::SeqCst), 0);
}

#[test]
fn the_errors_that_follow_a_loss_are_dropped_without_a_sound() {
    let Some(gpu) = started() else { return };
    let (watch, _) = watched(&gpu);
    gpu.lose();
    refused_a_buffer(&gpu);
    refused_a_buffer(&gpu);
    assert!(watch.is_lost());
}
