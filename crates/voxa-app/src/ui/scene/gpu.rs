//! The scene on the graphics device: the volume, the off-screen picture it is drawn into,
//! and that picture as the interface shows it.

use std::sync::{Arc, Weak};

use egui_wgpu::{RenderState, wgpu};
use voxa_core::Volume;
use voxa_render::{PICTURE_FORMAT, RenderError, SceneParams, VolumePipeline};

/// The format of the scene's picture: 8-bit greys as they are to be seen, which is how the
/// interface reads a picture it is handed, and the format a picture is read back in.
const FORMAT: wgpu::TextureFormat = PICTURE_FORMAT;

/// The off-screen picture, and the name the interface knows it by.
struct Picture {
    view: wgpu::TextureView,
    size: [u32; 2],
    id: egui::TextureId,
}

/// What the picture was last drawn from: the scene, the size it is laid out at, and the
/// volume, so that it is drawn again only when one of them changes.
type Drawn = (SceneParams, [f32; 2]);

/// What the scene keeps on the graphics device between frames.
pub struct SceneGpu {
    pipeline: VolumePipeline,
    picture: Option<Picture>,
    /// The volume that is uploaded now, so it is uploaded once and not every frame.
    uploaded: Weak<Volume>,
    drawn: Option<Drawn>,
}

impl SceneGpu {
    /// A pipeline drawing into a picture of its own, with nothing uploaded yet.
    pub fn new(render: &RenderState) -> Self {
        Self {
            pipeline: VolumePipeline::new(&render.device, FORMAT),
            picture: None,
            uploaded: Weak::new(),
            drawn: None,
        }
    }

    /// Put a volume on the graphics device, unless it is the one already there. Says how
    /// many times it had to be halved to fit when it was put there now; `None` when it was
    /// there already or the device has no room for it at all.
    pub fn hold(&mut self, render: &RenderState, volume: &Arc<Volume>) -> Option<u32> {
        if Weak::ptr_eq(&self.uploaded, &Arc::downgrade(volume)) {
            return None;
        }
        let halved = self
            .pipeline
            .upload(&render.device, &render.queue, volume)
            .ok()?;
        self.uploaded = Arc::downgrade(volume);
        self.drawn = None;
        Some(halved)
    }

    /// Draw the scene into a picture of its own, `pixels` wide and high, and give it back as
    /// red, green, blue, alpha bytes: for saving to a file. The picture on the screen is drawn
    /// afresh afterwards.
    pub fn snapshot(
        &mut self,
        render: &RenderState,
        params: &SceneParams,
        viewport: [f32; 2],
        pixels: [u32; 2],
    ) -> Result<Vec<u8>, RenderError> {
        self.drawn = None;
        let (device, queue) = (&render.device, &render.queue);
        self.pipeline
            .picture(device, queue, params, viewport, pixels)
    }

    /// Take what was drawn as no longer worth showing: the scene has moved on.
    pub fn forget(&mut self) {
        self.drawn = None;
    }

    /// Whether a picture of a volume has been drawn, so there is something to see.
    pub fn has_picture(&self) -> bool {
        self.drawn.is_some()
    }

    /// Draw the scene into its picture, `pixels` wide and high, unless the picture already
    /// shows exactly this, and give the name the interface shows it by. `viewport` is the
    /// size the picture is laid out at, which is what the camera measures in.
    pub fn draw(
        &mut self,
        render: &RenderState,
        params: &SceneParams,
        viewport: [f32; 2],
        pixels: [u32; 2],
    ) -> egui::TextureId {
        let limit = render.device.limits().max_texture_dimension_2d;
        let pixels = pixels.map(|side| side.clamp(1, limit));
        let picture = match self.picture.take() {
            Some(picture) if picture.size == pixels => picture,
            old => {
                self.drawn = None;
                Self::picture_of(render, pixels, old.map(|old| old.id))
            }
        };
        let wanted = (params.clone(), viewport);
        if self.drawn.as_ref() != Some(&wanted) {
            self.pipeline.set_params(&render.queue, params, viewport);
            let describe = wgpu::CommandEncoderDescriptor::default();
            let mut encoder = render.device.create_command_encoder(&describe);
            self.draw_into(&mut encoder, &picture.view);
            render.queue.submit([encoder.finish()]);
            self.drawn = Some(wanted);
        }
        let id = picture.id;
        self.picture = Some(picture);
        id
    }

    /// A picture of a new size, under the name the old one had if there was one.
    fn picture_of(
        render: &RenderState,
        [width, height]: [u32; 2],
        named: Option<egui::TextureId>,
    ) -> Picture {
        let texture = render.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("voxa scene picture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let smooth = wgpu::FilterMode::Linear;
        let mut renderer = render.renderer.write();
        let id = match named {
            Some(id) => {
                renderer.update_egui_texture_from_wgpu_texture(&render.device, &view, smooth, id);
                id
            }
            None => renderer.register_native_texture(&render.device, &view, smooth),
        };
        Picture {
            view,
            size: [width, height],
            id,
        }
    }

    /// One render pass that clears the picture to black and draws the volume over it.
    fn draw_into(&self, encoder: &mut wgpu::CommandEncoder, picture: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("voxa scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: picture,
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
}
