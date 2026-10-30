//! The graphics device the window asks for: the card's own limits for volumes, and 16-bit
//! normalised textures where it has them, so that a volume is held as stored and in as few
//! pieces as the card allows. With `VOXA_ADAPTER=software` the window asks for the system's
//! software graphics instead, for checking Voxa as it runs where there is no usable card.

use std::sync::Arc;

use egui_wgpu::wgpu;

/// The variable that asks for the system's software graphics.
const ADAPTER: &str = "VOXA_ADAPTER";

/// Set the window's graphics options to ask for what volumes need.
pub(crate) fn ask_for_what_volumes_need(options: &mut egui_wgpu::WgpuConfiguration) {
    if let egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_setup {
        setup.device_descriptor = Arc::new(descriptor);
        if std::env::var(ADAPTER).is_ok_and(|asked| asked == "software") {
            setup.native_adapter_selector = Some(Arc::new(|adapters, _| {
                let software = |adapter: &&wgpu::Adapter| {
                    adapter.get_info().device_type == wgpu::DeviceType::Cpu
                };
                let found = adapters.iter().find(software).cloned();
                found.ok_or_else(|| "no software graphics adapter".to_owned())
            }));
        }
    }
}

/// What is asked of a device on `adapter`: what egui asks for, with the card's own limits
/// for volumes and its 16-bit normalised textures.
fn descriptor(adapter: &wgpu::Adapter) -> wgpu::DeviceDescriptor<'static> {
    let base = if adapter.get_info().backend == wgpu::Backend::Gl {
        wgpu::Limits::downlevel_webgl2_defaults()
    } else {
        wgpu::Limits::default()
    };
    wgpu::DeviceDescriptor {
        label: Some("egui wgpu device"),
        required_limits: limits(&base, &adapter.limits()),
        required_features: adapter.features() & wgpu::Features::TEXTURE_FORMAT_16BIT_NORM,
        ..Default::default()
    }
}

/// egui's limits (`base`, with its 8,192 for pictures, for a depth buffer at 4K and more),
/// with the card's own longest side of a 3D texture and largest buffer.
pub(crate) fn limits(base: &wgpu::Limits, card: &wgpu::Limits) -> wgpu::Limits {
    wgpu::Limits {
        max_texture_dimension_2d: 8192,
        max_texture_dimension_3d: card.max_texture_dimension_3d,
        max_buffer_size: card.max_buffer_size,
        ..base.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cards_own_longest_side_and_largest_buffer_are_asked_for() {
        let card = wgpu::Limits {
            max_texture_dimension_3d: 16384,
            max_buffer_size: 1 << 34,
            ..wgpu::Limits::default()
        };
        let asked = limits(&wgpu::Limits::default(), &card);
        assert_eq!(asked.max_texture_dimension_3d, 16384);
        assert_eq!(asked.max_buffer_size, 1 << 34);
        assert_eq!(asked.max_texture_dimension_2d, 8192, "as egui asks");
        let default = wgpu::Limits::default();
        assert_eq!(
            asked.max_bind_groups, default.max_bind_groups,
            "the rest as egui's"
        );
    }
}
