//! Putting a slice's image on the graphics device: its values as 32-bit numbers, or the whole
//! numbers of 16 bits the file stores, at half the bytes, made values in the shader.

use egui_wgpu::wgpu;

use super::{RenderError, SlicePipeline, Uploaded, single};

/// A slice's numbers, row by row, as [`SlicePipeline::upload_numbers`] takes them.
#[derive(Debug, Clone, Copy)]
pub enum SliceNumbers<'a> {
    /// The values themselves.
    Values(&'a [f32]),
    /// Whole numbers of 16 bits as the file stores them, read as signed when `signed`; each
    /// is worth `slope`, and a stored 0 is `intercept`.
    Whole {
        /// The numbers' bits.
        numbers: &'a [u16],
        /// Whether the top bit is the sign.
        signed: bool,
        /// What each step of a number is worth.
        slope: f64,
        /// The value of a stored 0.
        intercept: f64,
    },
}

impl SlicePipeline {
    /// Put an image on the graphics device, replacing any image already there.
    /// `values[r * cols + c]` is the value at row `r`, column `c`.
    ///
    /// # Errors
    ///
    /// As [`SlicePipeline::upload_numbers`].
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rows: u32,
        cols: u32,
        values: &[f32],
    ) -> Result<(), RenderError> {
        self.upload_numbers((device, queue), rows, cols, SliceNumbers::Values(values))
    }

    /// Put an image on the graphics device as `numbers` give it, replacing any image already
    /// there. Whole numbers are sent as they are, 2 bytes each, and made values as they are
    /// drawn.
    ///
    /// # Errors
    ///
    /// [`RenderError::FrameSize`] when the number of values is not `rows × cols` or the image
    /// has no size, and [`RenderError::TooLarge`] when a side is longer than the device
    /// accepts. The image already uploaded, if any, stays in place.
    pub fn upload_numbers(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        rows: u32,
        cols: u32,
        numbers: SliceNumbers<'_>,
    ) -> Result<(), RenderError> {
        let (count, bytes, format, stored): (usize, &[u8], _, _) = match numbers {
            SliceNumbers::Values(values) => (
                values.len(),
                bytemuck::cast_slice(values),
                wgpu::TextureFormat::R32Float,
                [0.0; 4],
            ),
            SliceNumbers::Whole {
                numbers,
                signed,
                slope,
                intercept,
            } => (
                numbers.len(),
                bytemuck::cast_slice(numbers),
                wgpu::TextureFormat::R16Uint,
                [
                    single(slope),
                    single(intercept),
                    f32::from(u8::from(signed)),
                    1.0,
                ],
            ),
        };
        let expected = u64::from(rows) * u64::from(cols);
        if expected == 0 || count as u64 != expected {
            return Err(RenderError::FrameSize {
                rows,
                cols,
                values: count,
            });
        }
        let limit = device.limits().max_texture_dimension_2d;
        if rows > limit || cols > limit {
            return Err(RenderError::TooLarge { rows, cols, limit });
        }
        let reusable = self
            .uploaded
            .take()
            .filter(|old| (old.rows, old.cols, old.texture.format()) == (rows, cols, format));
        let mut uploaded = reusable.unwrap_or_else(|| self.texture(device, [cols, rows], format));
        uploaded.stored = stored;
        let per_texel = format.block_copy_size(None).unwrap_or(4);
        queue.write_texture(
            uploaded.texture.as_image_copy(),
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(cols * per_texel),
                rows_per_image: Some(rows),
            },
            wgpu::Extent3d {
                width: cols,
                height: rows,
                depth_or_array_layers: 1,
            },
        );
        self.uploaded = Some(uploaded);
        Ok(())
    }

    /// A texture of the given size (columns, rows) and format, and the bindings that show it:
    /// the shader reads it where it reads that format, and nothing where it reads the other.
    fn texture(
        &self,
        device: &wgpu::Device,
        [cols, rows]: [u32; 2],
        format: wgpu::TextureFormat,
    ) -> Uploaded {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("volsa slice values"),
            size: wgpu::Extent3d {
                width: cols,
                height: rows,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let (no_values, no_numbers) = &self.shader.blank;
        let (values, numbers) = if format == wgpu::TextureFormat::R16Uint {
            (no_values, &view)
        } else {
            (&view, no_numbers)
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("volsa slice"),
            layout: &self.shader.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(values),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(numbers),
                },
            ],
        });
        Uploaded {
            texture,
            bind_group,
            rows,
            cols,
            stored: [0.0; 4],
        }
    }
}
