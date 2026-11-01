//! The adapter devices with no window come from, and the graphics instance it was found with.
//! This is the one place in Volsa's code that makes a graphics instance for such a device.

use std::sync::OnceLock;

use egui_wgpu::wgpu;

use crate::slice_pipeline::RenderError;

/// An adapter, with the graphics instance it was found with, which a window would be given
/// along with a device made on the adapter.
pub(super) struct Found {
    pub(super) instance: wgpu::Instance,
    pub(super) adapter: wgpu::Adapter,
}

impl Found {
    /// Start a device on the adapter as `wanted` says. The caller holds its turn, see
    /// [`super::STARTING`].
    pub(super) fn start_device(
        &self,
        wanted: &wgpu::DeviceDescriptor<'_>,
    ) -> Result<(wgpu::Device, wgpu::Queue), RenderError> {
        pollster::block_on(self.adapter.request_device(wanted))
            .map_err(|error| RenderError::Device(error.to_string()))
    }

    /// Start a device for drawing pictures and reading them back, with all the adapter can
    /// give. The caller holds its turn, see [`super::STARTING`].
    pub(super) fn start_headless_device(&self) -> Result<(wgpu::Device, wgpu::Queue), RenderError> {
        let wanted = wgpu::DeviceDescriptor {
            label: Some("volsa headless"),
            required_limits: self.adapter.limits(),
            // Volumes of 16-bit whole numbers are held exactly as stored where it can.
            required_features: self.adapter.features() & wgpu::Features::TEXTURE_FORMAT_16BIT_NORM,
            memory_hints: super::memory_hints(self.adapter.get_info().device_type),
            ..Default::default()
        };
        self.start_device(&wanted)
    }
}

/// What `found` holds, looked for the first time it is asked for among the adapters asked
/// for in turn. The caller holds its turn, see [`super::STARTING`].
pub(super) fn found_in<'a>(
    found: &'a OnceLock<Option<Found>>,
    in_software: &[bool],
) -> Result<&'a Found, RenderError> {
    found
        .get_or_init(|| find_adapter(in_software, wgpu::Backends::from_env()))
        .as_ref()
        .ok_or(RenderError::NoAdapter)
}

/// The backends a graphics instance with no window is given, from `asked`: what
/// `WGPU_BACKEND` named, or `None` if it was not set.
///
/// Asked for, those backends are used as they are, to try another device on a machine.
/// Otherwise OpenGL is left out on Windows, where a driver's OpenGL side can stop every
/// thread of the program from starting or ending once a thread that used it has ended, which
/// stopped a run of the tests for good now and then. Vulkan and DirectX 12 remain, and with
/// DirectX 12 WARP, the device emulated in software that machines with no graphics card
/// draw with. Elsewhere nothing is left out, as wgpu does by default.
pub(super) fn backends_without_a_window(asked: Option<wgpu::Backends>) -> wgpu::Backends {
    let without_opengl = wgpu::Backends::VULKAN | wgpu::Backends::DX12;
    asked.unwrap_or(if cfg!(windows) {
        without_opengl
    } else {
        wgpu::Backends::default()
    })
}

/// The backends looked on for the adapters `in_software` asks for, from `asked`, as
/// [`backends_without_a_window`] gives them, but for one case: on Windows, with nothing asked
/// for, software graphics alone are looked for on DirectX 12 alone. Windows' software graphics
/// is a DirectX 12 adapter, and looking for it on every kind of graphics woke the card's other
/// drivers while it was drawing for another device, which now and then never finished.
pub(super) fn backends_to_search(
    in_software: &[bool],
    asked: Option<wgpu::Backends>,
) -> wgpu::Backends {
    match asked {
        None if cfg!(windows) && in_software == [true] => wgpu::Backends::DX12,
        asked => backends_without_a_window(asked),
    }
}

/// The first adapter found of those asked for in turn, with the instance it was found with:
/// `false` asks for the machine's own, `true` for one emulated in software. Which backend
/// and which adapter can be chosen from outside, to try more than one graphics device on a
/// machine: `WGPU_BACKEND`, which is `asked_backends` here, and `WGPU_POWER_PREF`. This is
/// the one place in Volsa's code that makes a graphics instance for a device with no window,
/// and it leaves OpenGL out on Windows, see [`backends_without_a_window`]. Asked for software
/// graphics alone, on Windows with `WGPU_BACKEND` not set, it looks on DirectX 12 alone.
pub(super) fn find_adapter(
    in_software: &[bool],
    asked_backends: Option<wgpu::Backends>,
) -> Option<Found> {
    let from_outside = wgpu::InstanceDescriptor {
        backends: backends_to_search(in_software, asked_backends),
        ..wgpu::InstanceDescriptor::new_without_display_handle_from_env()
    };
    let instance = wgpu::Instance::new(from_outside);
    let adapter = |force_fallback_adapter| {
        let wanted = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::from_env().unwrap_or_default(),
            force_fallback_adapter,
            ..Default::default()
        };
        pollster::block_on(instance.request_adapter(&wanted)).ok()
    };
    let adapter = in_software.iter().find_map(|&software| adapter(software))?;
    Some(Found { instance, adapter })
}
