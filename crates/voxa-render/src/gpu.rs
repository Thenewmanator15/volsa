//! A graphics device with no window, and pictures drawn on it and read back.

use std::sync::{Mutex, OnceLock, PoisonError, mpsc};

use egui_wgpu::wgpu;

use crate::slice_pipeline::RenderError;

/// The format of the picture drawn: 8-bit red, green, blue and alpha, with no gamma curve, so
/// the grey read back is the grey the shader wrote.
pub(crate) const TARGET: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const BYTES_PER_PIXEL: u32 = 4;

/// Devices started without a window all come from one adapter, and start one at a time. A
/// run of the tests on many threads, each looking for an adapter of its own and starting a
/// device on it, stopped for good on one machine's driver. One adapter was enough to cure
/// it there; taking turns as well costs little, since starting a device on an adapter
/// already found is quick.
static STARTING: Mutex<()> = Mutex::new(());

/// The adapter those devices come from, looked for once: the search is most of the time a
/// start would otherwise take.
static ADAPTER: OnceLock<Option<wgpu::Adapter>> = OnceLock::new();

/// A graphics device with no window.
pub(crate) struct Gpu {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
}

impl Gpu {
    /// A device that is already running, such as the one a window draws with.
    pub(crate) fn on(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
        }
    }

    /// Start a graphics device: the machine's own adapter if it has one, otherwise one
    /// emulated in software.
    ///
    /// # Errors
    ///
    /// [`RenderError::NoAdapter`] when the machine has neither, and [`RenderError::Device`]
    /// when the adapter found will not start.
    pub(crate) fn start() -> Result<Self, RenderError> {
        let _turn = STARTING.lock().unwrap_or_else(PoisonError::into_inner);
        let adapter = ADAPTER
            .get_or_init(find_adapter)
            .as_ref()
            .ok_or(RenderError::NoAdapter)?;
        let wanted = wgpu::DeviceDescriptor {
            label: Some("voxa headless"),
            required_limits: adapter.limits(),
            ..Default::default()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wanted))
            .map_err(|error| RenderError::Device(error.to_string()))?;
        Ok(Self { device, queue })
    }

    /// Clear a picture `size` (width, height) pixels large to black, let `draw` draw over
    /// it, and return it as red, green, blue, alpha bytes, row by row from the top.
    ///
    /// # Errors
    ///
    /// [`RenderError::EmptyTarget`] when either side of `size` is zero,
    /// [`RenderError::TooLarge`] when it is more than the device can hold, and
    /// [`RenderError::Readback`] when the picture cannot be copied back.
    pub(crate) fn picture(
        &self,
        size: [u32; 2],
        draw: impl FnOnce(&mut wgpu::RenderPass<'_>),
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

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        draw_into(&mut encoder, &target, draw);
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

    /// Every 16-bit value of a 3D texture, columns fastest, copied back from the device.
    #[cfg(test)]
    pub(crate) fn halves_of(&self, texture: &wgpu::Texture) -> Vec<u16> {
        let extent = texture.size();
        let row_bytes = extent.width * 2;
        let padded_row = row_bytes.next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let rows = extent.height * extent.depth_or_array_layers;
        let copied = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("voxa headless copy"),
            size: u64::from(padded_row) * u64::from(rows),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &copied,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: Some(extent.height),
                },
            },
            extent,
        );
        self.queue.submit([encoder.finish()]);
        let bytes = self
            .read_back(&copied, row_bytes as usize, padded_row as usize)
            .expect("the copy comes back");
        let (pairs, _) = bytes.as_chunks::<2>();
        pairs.iter().map(|pair| u16::from_le_bytes(*pair)).collect()
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

/// One render pass that clears the picture to black and lets `draw` draw over it.
/// The machine's own adapter if it has one, otherwise one emulated in software. Which
/// backend and which adapter can be chosen from outside, to try more than one graphics
/// device on a machine: `WGPU_BACKEND` and `WGPU_POWER_PREF`.
fn find_adapter() -> Option<wgpu::Adapter> {
    let from_outside = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    let instance = wgpu::Instance::new(from_outside);
    let adapter = |force_fallback_adapter| {
        let wanted = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::from_env().unwrap_or_default(),
            force_fallback_adapter,
            ..Default::default()
        };
        pollster::block_on(instance.request_adapter(&wanted)).ok()
    };
    adapter(false).or_else(|| adapter(true))
}

fn draw_into(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::Texture,
    draw: impl FnOnce(&mut wgpu::RenderPass<'_>),
) {
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
    draw(&mut pass);
}
