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
    let mut largest = BlockMaxima::new(dims);
    largest.add(0, halves);
    largest.finish()
}

/// The largest texel in each block of a volume, gathered a run of whole slices at a time as
/// the volume is handed to a device that cannot work it out itself. Texels are compared as
/// whole numbers: right for 16-bit normalised texels, and for 16-bit floats from 0 to 1.
pub(crate) struct BlockMaxima {
    dims: [u32; 3],
    own: Vec<u16>,
}

impl BlockMaxima {
    /// Nothing gathered yet, for a volume of `dims`.
    pub(crate) fn new(dims: [u32; 3]) -> Self {
        let block = crate::prepared::BLOCK as usize;
        let [across, down, deep] = dims.map(|count| (count as usize).div_ceil(block));
        Self {
            dims,
            own: vec![0; across * down * deep],
        }
    }

    /// Take in the texels of whole slices, the first of them slice `first`.
    pub(crate) fn add(&mut self, first: u32, texels: &[u16]) {
        let [cols, rows, _] = self.dims.map(|count| count as usize);
        let block = crate::prepared::BLOCK as usize;
        let [across, down, _] = self.dims.map(|count| (count as usize).div_ceil(block));
        if cols * rows == 0 {
            return;
        }
        for (within, slice) in texels.chunks(cols * rows).enumerate() {
            let slice_at = first as usize + within;
            for (row, line) in slice.chunks(cols).enumerate() {
                let start = (slice_at / block * down + row / block) * across;
                for (col, texel) in line.iter().enumerate() {
                    if let Some(largest) = self.own.get_mut(start + col / block) {
                        *largest = (*largest).max(*texel);
                    }
                }
            }
        }
    }

    /// The largest texel of each block and of the blocks touching it.
    pub(crate) fn finish(self) -> Vec<u16> {
        let block = crate::prepared::BLOCK as usize;
        let [across, down, deep] = self.dims.map(|count| (count as usize).div_ceil(block));
        let own = self.own;
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
}
