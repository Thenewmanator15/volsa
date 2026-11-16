//! Whether a graphics card answers on Vulkan, for the window to look there alone.

use egui_wgpu::wgpu;

/// Whether a graphics card, not software, answers on Vulkan. The window looks on Vulkan alone
/// where one does, which is what it chooses among every backend anyway: looking on the others
/// too loads every card's DirectX or OpenGL driver for devices that are never chosen. Where
/// none does (no driver, or a remote session) the window looks on every backend. Only Vulkan
/// is looked on here, so no OpenGL driver is woken.
pub fn has_a_vulkan_card() -> bool {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle_from_env()
    });
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN));
    adapters
        .iter()
        .any(|adapter| adapter.get_info().device_type != wgpu::DeviceType::Cpu)
}
