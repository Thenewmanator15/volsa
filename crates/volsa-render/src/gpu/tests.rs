//! Which graphics backends a device with no window is given, and that the instance the shared
//! adapter was found with is the one that choice made; how such a device sets memory aside,
//! and how a picture is read back from it.

#[cfg(windows)]
use egui_wgpu::wgpu::Backend;
use egui_wgpu::wgpu::{self, Backends};

use super::found::backends_without_a_window;
#[cfg(windows)]
use super::found::{backends_to_search, find_adapter};
use super::*;

/// Every backend that can be named, and some pairs, for what `WGPU_BACKEND` may have asked for.
fn askable() -> Vec<Backends> {
    vec![
        Backends::GL,
        Backends::VULKAN,
        Backends::DX12,
        Backends::GL | Backends::VULKAN,
        Backends::VULKAN | Backends::DX12,
        Backends::all(),
    ]
}

#[cfg(windows)]
#[test]
fn on_windows_with_nothing_asked_for_opengl_is_left_out_and_vulkan_and_dx12_remain() {
    let chosen = backends_without_a_window(None);
    assert!(
        chosen.contains(Backends::VULKAN | Backends::DX12),
        "{chosen:?}"
    );
    assert!(!chosen.intersects(Backends::GL), "{chosen:?}");
}

#[cfg(not(windows))]
#[test]
fn off_windows_with_nothing_asked_for_nothing_is_left_out() {
    assert_eq!(backends_without_a_window(None), Backends::default());
}

#[test]
fn what_was_asked_for_is_used_as_it_is_even_opengl() {
    for asked in askable() {
        assert_eq!(backends_without_a_window(Some(asked)), asked);
    }
}

#[cfg(windows)]
#[test]
fn an_instance_found_with_nothing_asked_for_has_no_opengl_adapter_to_find() {
    // Looking for an adapter takes its turn, like starting a device does.
    let _turn = STARTING.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(found) = find_adapter(&[false, true], None) else {
        eprintln!("skipped: no graphics adapter here, not even a software one");
        return;
    };
    let adapters = pollster::block_on(found.instance.enumerate_adapters(Backends::all()));
    assert!(
        !adapters.is_empty(),
        "the adapter was found on this instance"
    );
    for adapter in adapters {
        let info = adapter.get_info();
        assert_ne!(
            info.backend,
            Backend::Gl,
            "{} is an OpenGL adapter",
            info.name
        );
    }
    let backend = found.adapter.get_info().backend;
    assert_ne!(
        backend,
        Backend::Gl,
        "the adapter found is not an OpenGL one"
    );
}

#[cfg(windows)]
#[test]
fn on_windows_software_graphics_alone_are_looked_for_on_dx12_alone_unless_asked() {
    assert_eq!(backends_to_search(&[true], None), Backends::DX12);
    assert_eq!(
        backends_to_search(&[false, true], None),
        backends_without_a_window(None)
    );
    for asked in askable() {
        assert_eq!(backends_to_search(&[true], Some(asked)), asked);
    }
}

#[test]
fn rows_are_copied_without_their_padding() {
    let mut padded = Vec::new();
    for row in [0u8, 10, 20] {
        padded.extend((row..row + 5).chain([9, 9, 9]));
    }
    let picture = unpadded(&padded, 5, 8, 3);
    let wanted: Vec<u8> = [0u8, 10, 20].iter().flat_map(|&row| row..row + 5).collect();
    assert_eq!(picture, wanted);
}

#[test]
fn only_the_whole_rows_held_are_copied() {
    let padded: Vec<u8> = (0..20).collect();
    assert_eq!(
        unpadded(&padded, 5, 8, 3),
        [0, 1, 2, 3, 4, 8, 9, 10, 11, 12]
    );
}

/// A way of starting a device with no window.
type Start = fn() -> Result<super::Gpu, RenderError>;

/// What a device reserves once one small buffer is made on it, in bytes; `None` when there
/// is no such device here, or it does not say.
fn reserved_for_a_small_buffer(gpu: Result<super::Gpu, RenderError>) -> Option<u64> {
    let gpu = gpu.ok()?;
    let _small = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("small"),
        size: 1 << 20,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    Some(gpu.device.generate_allocator_report()?.total_reserved_bytes)
}

#[test]
fn a_device_reserves_memory_in_small_blocks_on_a_card_and_on_software_graphics_alike() {
    // wgpu's default reserves 192 MiB for the first small thing made, and Windows counts it
    // as the program's; with software graphics, the only device a computer with no card has,
    // all of it is the computer's own memory. One device at a time, as the window has one.
    let starts: [(&str, Start); 2] = [
        ("the adapter devices with no window use", super::Gpu::start),
        ("software graphics", super::Gpu::start_in_software),
    ];
    for (which, start) in starts {
        let Some(reserved) = reserved_for_a_small_buffer(start()) else {
            eprintln!("skipped for {which}: no such device here, or it says nothing");
            continue;
        };
        assert!(
            reserved <= 64 << 20,
            "{which}: {} MiB reserved",
            reserved >> 20
        );
    }
}

#[test]
fn every_device_is_asked_to_set_memory_aside_in_small_blocks() {
    assert!(matches!(
        super::memory_hints(),
        wgpu::MemoryHints::MemoryUsage
    ));
}
