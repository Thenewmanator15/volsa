//! Which blocks of a label volume hold any part at all, worked out by the graphics device
//! from the labels it holds, so that a line of sight can step over the blocks that hold none.

use egui_wgpu::wgpu;

use super::{IntoSlices, if_the_device_can, texture};

/// The side, in voxels, of the blocks a label volume is divided into.
const PART_BLOCK: u32 = 8;
/// A block's figure is kept as a volume's values are, which every device can draw into.
const HALVES: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// Works out, on the device, which blocks of a label volume have a labelled voxel in them.
pub(crate) struct FilledMaker {
    any: IntoSlices,
}

impl FilledMaker {
    /// `None` on a device that cannot draw into the slices of a 3D texture.
    pub(crate) fn new(device: &wgpu::Device) -> Option<Self> {
        let whole_numbers = wgpu::TextureSampleType::Uint;
        if_the_device_can(device, || Self {
            any: IntoSlices::new(
                device,
                include_str!("filled.wgsl"),
                "any_in_block",
                HALVES,
                whole_numbers,
            ),
        })
    }

    /// For a label volume of `dims` voxels whose part numbers `parts` reads: a texture of one
    /// texel a block, 1 where the block holds a part and 0 where it holds none. The work is
    /// done by the device after this returns.
    pub(crate) fn make(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        parts: &wgpu::TextureView,
        dims: [u32; 3],
    ) -> wgpu::Texture {
        let blocks = dims.map(|count| count.div_ceil(PART_BLOCK).max(1));
        let drawn_and_read = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC;
        let filled = texture(device, "volsa label blocks", blocks, HALVES, drawn_and_read);
        let [_, _, slices] = blocks;
        self.any.draw((device, queue), parts, &filled, 0..slices);
        filled
    }
}
