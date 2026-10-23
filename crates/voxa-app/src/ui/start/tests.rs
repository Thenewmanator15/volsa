//! Tests of the graphics backends the window starts on: never OpenGL on Windows, unless it is
//! named from outside, and still a device drawn in software where there is no card to use.

use std::pin::pin;
use std::task::{Context, Poll, Waker};

use egui_wgpu::wgpu::{self, Backend, Backends, DeviceType};
use egui_wgpu::{WgpuSetup, WgpuSetupCreateNew};

use super::{window_backends, with_backends};

#[test]
fn on_windows_the_window_starts_on_vulkan_or_directx_12_and_never_tries_opengl() {
    let chosen = window_backends(None);
    if cfg!(windows) {
        assert_eq!(chosen, Backends::VULKAN | Backends::DX12);
        assert!(!chosen.contains(Backends::GL));
    } else {
        // Elsewhere eframe's own choice stands: OpenGL is what some machines there have.
        assert_eq!(chosen, Backends::PRIMARY | Backends::GL);
    }
}

#[test]
fn backends_named_from_outside_are_the_ones_tried() {
    // What `WGPU_BACKEND` names, read by wgpu, is passed in and kept as it is, OpenGL included.
    for named in [
        Backends::GL,
        Backends::DX12,
        Backends::VULKAN,
        Backends::VULKAN | Backends::GL,
    ] {
        assert_eq!(window_backends(Some(named)), named);
    }
}

#[test]
fn the_options_the_window_opens_with_carry_the_backends_and_nothing_else_changes() {
    let before = eframe::NativeOptions::default();
    let after = with_backends(eframe::NativeOptions::default(), Backends::DX12);
    let setup = |options: &eframe::NativeOptions| match &options.wgpu_options.wgpu_setup {
        WgpuSetup::CreateNew(create) => (
            create.instance_descriptor.backends,
            create.power_preference,
            create.instance_descriptor.flags,
        ),
        WgpuSetup::Existing(_) => panic!("eframe makes its own device"),
    };
    let (backends, power, flags) = setup(&after);
    assert_eq!(backends, Backends::DX12);
    let (_, power_before, flags_before) = setup(&before);
    assert_eq!((power, flags), (power_before, flags_before));
    assert_eq!(
        format!("{:?}", after.wgpu_options.surface),
        format!("{:?}", before.wgpu_options.surface)
    );
}

/// What a future gives, waited for on this thread. wgpu's own futures are ready at once.
fn now<T>(future: impl Future<Output = T>) -> T {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(given) = future.as_mut().poll(&mut context) {
            return given;
        }
        std::thread::yield_now();
    }
}

#[test]
fn on_windows_with_no_card_to_use_the_window_still_has_directx_12_drawn_in_software() {
    if !cfg!(windows) {
        return;
    }
    // The adapters the window may choose among, with the backends it starts on. wgpu chooses
    // a software one only when no other is there; this is the one it would choose then.
    let mut wanted = wgpu::InstanceDescriptor::new_without_display_handle();
    wanted.backends = window_backends(None);
    let instance = wgpu::Instance::new(wanted);
    let adapters = now(instance.enumerate_adapters(window_backends(None)));
    let backends: Vec<Backend> = adapters.iter().map(|one| one.get_info().backend).collect();
    assert!(!backends.contains(&Backend::Gl), "{backends:?}");
    let in_software = adapters.iter().find(|one| {
        let info = one.get_info();
        info.device_type == DeviceType::Cpu && info.backend == Backend::Dx12
    });
    let in_software = in_software.expect("Windows' own DirectX 12 drawn in software");
    // It starts the device eframe asks for, with the limits it asks for.
    let asked = WgpuSetupCreateNew::without_display_handle().device_descriptor;
    let started = now(in_software.request_device(&(*asked)(in_software)));
    assert!(started.is_ok(), "{:?}", started.err());
}
