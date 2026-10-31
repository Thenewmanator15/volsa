//! Which graphics backends a device with no window is given, and that the instance the shared
//! adapter was found with is the one that choice made.

use egui_wgpu::wgpu::{Backend, Backends};

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
