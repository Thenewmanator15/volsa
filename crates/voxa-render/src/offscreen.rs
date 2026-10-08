//! Drawing a slice without a window, for tests and thumbnails.

use std::sync::mpsc;

use egui_wgpu::wgpu;
use voxa_core::WindowLevel;

use crate::slice_pipeline::{RenderError, SlicePipeline};
use crate::view::ViewTransform;

/// The format of the picture drawn: 8-bit red, green, blue and alpha, with no gamma curve, so
/// the grey read back is the grey the shader wrote.
const TARGET: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const BYTES_PER_PIXEL: u32 = 4;

/// A graphics device with no window, holding one [`SlicePipeline`].
pub struct Headless {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: SlicePipeline,
}

impl Headless {
    /// Start a graphics device: the machine's own adapter if it has one, otherwise one
    /// emulated in software.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoAdapter`] when the machine has neither, and [`RenderError::Device`]
    /// when the adapter found will not start.
    pub fn new() -> Result<Self, RenderError> {
        let instance = wgpu::Instance::default();
        let adapter = |force_fallback_adapter| {
            let wanted = wgpu::RequestAdapterOptions {
                force_fallback_adapter,
                ..Default::default()
            };
            pollster::block_on(instance.request_adapter(&wanted)).ok()
        };
        let adapter = adapter(false)
            .or_else(|| adapter(true))
            .ok_or(RenderError::NoAdapter)?;
        let wanted = wgpu::DeviceDescriptor {
            label: Some("voxa headless"),
            required_limits: adapter.limits(),
            ..Default::default()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wanted))
            .map_err(|error| RenderError::Device(error.to_string()))?;
        let pipeline = SlicePipeline::new(&device, TARGET);
        Ok(Self {
            device,
            queue,
            pipeline,
        })
    }

    /// Put an image on the graphics device; see [`SlicePipeline::upload`].
    ///
    /// # Errors
    ///
    /// As [`SlicePipeline::upload`].
    pub fn upload(&mut self, rows: u32, cols: u32, values: &[f32]) -> Result<(), RenderError> {
        self.pipeline
            .upload(&self.device, &self.queue, rows, cols, values)
    }

    /// Draw the uploaded image into a picture `size` (width, height) pixels large and return
    /// it as red, green, blue, alpha bytes, row by row from the top. Square pixels are
    /// assumed. With nothing uploaded the picture is black.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTarget`] when either side of `size` is zero,
    /// [`RenderError::TooLarge`] when it is more than the device can hold, and
    /// [`RenderError::Readback`] when the picture cannot be copied back.
    pub fn render(
        &mut self,
        wl: WindowLevel,
        view: &ViewTransform,
        size: [u32; 2],
    ) -> Result<Vec<u8>, RenderError> {
        let [width, height] = size;
        if width == 0 || height == 0 {
            return Err(RenderError::EmptyTarget);
        }
        let limit = self.device.limits().max_texture_dimension_2d;
        if width > limit || height > limit {
            return Err(RenderError::TooLarge {
                rows: height,
                cols: width,
                limit,
            });
        }
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("voxa headless picture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        // Rows are copied out padded to the length the device requires.
        let row_bytes = width * BYTES_PER_PIXEL;
        let padded_row = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let copied = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("voxa headless copy"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        self.pipeline
            .set_params(&self.queue, wl, view, [width as f32, height as f32], 1.0);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        self.draw_into(&mut encoder, &target);
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &copied,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(height),
                },
            },
            extent,
        );
        self.queue.submit([encoder.finish()]);
        self.read_back(&copied, row_bytes as usize, padded_row as usize)
    }

    /// One render pass that clears the picture to black and draws the slice over it.
    fn draw_into(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::Texture) {
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("voxa headless"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.pipeline.draw(&mut pass);
    }

    /// Wait for the copy to finish and take the picture out of it, dropping the row padding.
    fn read_back(
        &self,
        copied: &wgpu::Buffer,
        row_bytes: usize,
        padded_row: usize,
    ) -> Result<Vec<u8>, RenderError> {
        let failed = |why: String| RenderError::Readback(why);
        let (done, mapped) = mpsc::channel();
        copied.map_async(wgpu::MapMode::Read, .., move |result| {
            // The receiver is waiting just below; if it has gone there is no one to tell.
            let _ = done.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| failed(error.to_string()))?;
        mapped
            .recv()
            .map_err(|error| failed(error.to_string()))?
            .map_err(|error| failed(error.to_string()))?;
        let bytes = copied
            .get_mapped_range(..)
            .map_err(|error| failed(error.to_string()))?;
        let picture = bytes
            .chunks(padded_row)
            .flat_map(|row| row.iter().take(row_bytes).copied())
            .collect();
        Ok(picture)
    }
}

/// Draw one image into a picture in a single call: start a device, upload, draw, read back.
/// `size` is the picture's width and height.
///
/// # Errors
///
/// Any [`RenderError`]: no adapter, a frame whose values do not match its size, a picture of
/// no size, or a failed read-back.
pub fn render_to_rgba(
    rows: u32,
    cols: u32,
    values: &[f32],
    wl: WindowLevel,
    view: &ViewTransform,
    size: [u32; 2],
) -> Result<Vec<u8>, RenderError> {
    let mut gpu = Headless::new()?;
    gpu.upload(rows, cols, values)?;
    gpu.render(wl, view, size)
}

#[cfg(test)]
mod tests;
