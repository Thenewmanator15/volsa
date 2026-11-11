//! The largest value in each block of a volume, worked out by the graphics device from the
//! volume it has just been given, in place of a pass over every voxel on the processor.

use egui_wgpu::wgpu;

use super::{IntoSlices, REALS, if_the_device_can, texture};

const SOURCE: &str = include_str!("blocks.wgsl");
/// The blocks are kept as the volume's values are.
const HALVES: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// Works out, on the device, the largest value in each block and in the blocks touching it.
pub(crate) struct BlocksMaker {
    within: IntoSlices,
    about: IntoSlices,
}

impl BlocksMaker {
    /// `None` on a device that cannot draw into the slices of a 3D texture.
    pub(crate) fn new(device: &wgpu::Device) -> Option<Self> {
        if_the_device_can(device, || Self {
            within: IntoSlices::new(device, SOURCE, "largest_in_block", HALVES, REALS),
            about: IntoSlices::new(device, SOURCE, "largest_about", HALVES, REALS),
        })
    }

    /// The largest value in each block of the volume whose values `values` reads, for a
    /// volume of `blocks` blocks each way: a texture the volume's shader reads beside the
    /// values themselves. The work is done by the device after this returns.
    pub(crate) fn make(
        &self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        values: &wgpu::TextureView,
        blocks: [u32; 3],
    ) -> wgpu::Texture {
        let drawn_and_read = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC;
        let [_, _, slices] = blocks;
        // Each block's own largest value first, then spread to the blocks about it.
        let own = texture(
            device,
            "voxa volume blocks, each alone",
            blocks,
            HALVES,
            drawn_and_read,
        );
        self.within.draw((device, queue), values, &own, slices);
        let largest = texture(
            device,
            "voxa volume block maxima",
            blocks,
            HALVES,
            drawn_and_read,
        );
        let own_view = own.create_view(&wgpu::TextureViewDescriptor::default());
        self.about
            .draw((device, queue), &own_view, &largest, slices);
        // Given back once the device has drawn from it, not before.
        own.destroy();
        largest
    }
}

/// What [`BlocksMaker`] makes, worked out here from the values as they are stored, for a
/// device that cannot: the largest value in each block and the blocks touching it. Stored
/// values run from 0 to 1 as 16-bit floating point, whose bits rise as the values do, so
/// the largest is found without unpacking any of them.
pub(crate) fn largest_in_blocks(halves: &[u16], dims: [u32; 3]) -> Vec<u16> {
    let [cols, rows, _] = dims.map(|count| count as usize);
    let block = crate::prepared::BLOCK as usize;
    let [across, down, deep] = dims.map(|count| (count as usize).div_ceil(block));
    let mut own = vec![0_u16; across * down * deep];
    if cols * rows == 0 {
        return own;
    }
    for (slice, slab) in halves.chunks(cols * rows).enumerate() {
        for (row, line) in slab.chunks(cols).enumerate() {
            let first = (slice / block * down + row / block) * across;
            for (col, value) in line.iter().enumerate() {
                if let Some(largest) = own.get_mut(first + col / block) {
                    *largest = (*largest).max(*value);
                }
            }
        }
    }
    let about = |at: usize, count: usize| at.saturating_sub(1)..(at + 2).min(count);
    let mut spread = Vec::with_capacity(own.len());
    for slice in 0..deep {
        for row in 0..down {
            for col in 0..across {
                let near = about(slice, deep)
                    .flat_map(|slice| {
                        about(row, down).map(move |row| (slice * down + row) * across)
                    })
                    .flat_map(|first| about(col, across).map(move |col| first + col));
                let largest = near.filter_map(|at| own.get(at)).max();
                spread.push(largest.copied().unwrap_or(0));
            }
        }
    }
    spread
}
