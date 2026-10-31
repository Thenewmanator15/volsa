use egui_wgpu::wgpu;

use super::unpadded;

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

#[test]
fn the_device_reserves_memory_in_small_blocks() {
    // wgpu's default reserves 128 MiB or more of the card's memory, and 64 MiB of the
    // computer's, for the first small thing made, and Windows counts it as the program's.
    let Ok(gpu) = super::Gpu::start() else {
        return;
    };
    let _small = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("small"),
        size: 1 << 20,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let Some(report) = gpu.device.generate_allocator_report() else {
        return;
    };
    assert!(
        report.total_reserved_bytes <= 64 << 20,
        "{} MiB reserved",
        report.total_reserved_bytes >> 20
    );
}

#[test]
fn a_card_sets_memory_aside_in_small_blocks_and_software_graphics_as_wgpu_would() {
    let small = |kind| matches!(super::memory_hints(kind), wgpu::MemoryHints::MemoryUsage);
    assert!(small(wgpu::DeviceType::DiscreteGpu));
    assert!(small(wgpu::DeviceType::IntegratedGpu));
    assert!(
        !small(wgpu::DeviceType::Cpu),
        "software graphics keep wgpu's own"
    );
}
