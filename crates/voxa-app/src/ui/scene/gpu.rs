//! The scene on the graphics device: the volume, the off-screen picture it is drawn into,
//! and that picture as the interface shows it.

use std::sync::{Arc, Weak};
use std::time::Duration;

use egui_wgpu::{RenderState, wgpu};
use voxa_core::Volume;
use voxa_render::{
    LabelsOnDevice, Look, PICTURE_FORMAT, PartsPipeline, Prepared, RenderError, SceneParams,
    VolumePipeline, timed_submit,
};

use super::pace::{TIME_EVERY, TIMING_TRIES};

/// The format of the scene's picture: 8-bit greys as they are to be seen, which is how the
/// interface reads a picture it is handed, and the format a picture is read back in.
const FORMAT: wgpu::TextureFormat = PICTURE_FORMAT;

/// The off-screen picture, and the name the interface knows it by.
struct Picture {
    view: wgpu::TextureView,
    size: [u32; 2],
    id: egui::TextureId,
}

/// What the picture was last drawn from: the scene, the size it is laid out at, and how
/// the labels stood, so that it is drawn again only when one of them changes.
type Drawn = (SceneParams, [f32; 2], u64);

/// What a timing is of: the picture's size in pixels, the look, and whether it was drawn
/// as labelled solids.
type Timed = ([u32; 2], Look, bool);

/// How long the device takes over a picture drawn in full.
#[derive(Default)]
struct Timing {
    /// The last time measured, and what picture it was of.
    took: Option<(Timed, Duration)>,
    /// How many pictures in full have been drawn since one was timed, and how many
    /// times a picture could not be timed because the device was busy.
    untimed: u32,
    tries: u32,
    /// Whether the picture before this one was drawn in full.
    was_full: bool,
}

/// What the scene keeps on the graphics device between frames.
pub struct SceneGpu {
    pipeline: VolumePipeline,
    /// Draws the labelled parts as solids, for the Parts look.
    parts: PartsPipeline,
    /// A number that moves on when the labels on the device change.
    labels: u64,
    picture: Option<Picture>,
    /// The volume the device was last given, so it is given once and not every frame.
    given: Weak<Volume>,
    drawn: Option<Drawn>,
    timing: Timing,
}

impl SceneGpu {
    /// A pipeline drawing into a picture of its own, with nothing uploaded yet.
    pub fn new(render: &RenderState) -> Self {
        Self {
            pipeline: VolumePipeline::new(&render.device, FORMAT),
            parts: PartsPipeline::new(&render.device, FORMAT),
            labels: 0,
            picture: None,
            given: Weak::new(),
            drawn: None,
            timing: Timing::default(),
        }
    }

    /// Whether this volume is the one the graphics device was last given.
    pub fn holds(&self, volume: &Arc<Volume>) -> bool {
        Weak::ptr_eq(&self.given, &Arc::downgrade(volume))
    }

    /// Whether there is a volume on the graphics device to draw.
    pub fn has_volume(&self) -> bool {
        self.pipeline.has_volume()
    }

    /// The pipeline that holds the volume.
    pub fn volume_pipeline(&self) -> &VolumePipeline {
        &self.pipeline
    }

    /// Draw the Parts look from these labels, or have none to draw from. `stamp` moves on
    /// whenever the labels, or how their parts look, change.
    pub fn label(&mut self, render: &RenderState, with: Option<&LabelsOnDevice>, stamp: u64) {
        self.parts.show(&render.device, with);
        self.labels = stamp;
    }

    /// Whether a scene is drawn as labelled solids: that is its look, and there are labels.
    fn as_parts(&self, params: &SceneParams) -> bool {
        params.look == Look::Parts && self.parts.has_labels()
    }

    /// Put a volume on the graphics device, unless it is the one already given. `ready` is
    /// the volume as a worker made it ready, if it did: handing that over is quick. Without
    /// it, or if it does not fit this device, the volume is made ready here. Says how many
    /// times it was halved to fit when it was put there now. If the device has no room for
    /// it at all, nothing is left there to draw, and it is not tried again.
    pub fn hold(
        &mut self,
        render: &RenderState,
        volume: &Arc<Volume>,
        ready: Option<Prepared>,
    ) -> Option<u32> {
        if self.holds(volume) {
            return None;
        }
        let (device, queue) = (&render.device, &render.queue);
        let quick =
            ready.and_then(|ready| self.pipeline.upload_prepared(device, queue, &ready).ok());
        let halved = quick.or_else(|| self.pipeline.upload(device, queue, volume).ok());
        self.given = Arc::downgrade(volume);
        self.drawn = None;
        self.timing = Timing::default();
        halved
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
        if self.as_parts(params) {
            return self.parts.picture(device, queue, params, viewport, pixels);
        }
        self.pipeline
            .picture(device, queue, params, viewport, pixels)
    }

    /// How long the device took over the last picture of `pixels` it drew in full, as
    /// this scene is drawn. `None` when it has timed none since the volume, the size or
    /// the look changed.
    pub fn full_took(&self, pixels: [u32; 2], params: &SceneParams) -> Option<Duration> {
        let wanted = (pixels, params.look, self.as_parts(params));
        let (timed, took) = self.timing.took.as_ref()?;
        (*timed == wanted).then_some(*took)
    }

    /// Whether a picture in full has been drawn and not timed, the device having been
    /// busy, and it is worth drawing one again to time it.
    pub fn wants_timing(&self, pixels: [u32; 2], params: &SceneParams) -> bool {
        let untimed = self.full_took(pixels, params).is_none();
        params.quality >= 1.0 && untimed && self.timing.tries < TIMING_TRIES
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
        let wanted = (params.clone(), viewport, self.labels);
        // Drawn again when anything has changed, and once more when a full picture has
        // still to be timed.
        if self.drawn.as_ref() != Some(&wanted) || self.wants_timing(pixels, params) {
            let as_parts = self.as_parts(params);
            if as_parts {
                self.parts.set_params(&render.queue, params, viewport);
            } else {
                self.pipeline.set_params(&render.queue, params, viewport);
            }
            let describe = wgpu::CommandEncoderDescriptor::default();
            let mut encoder = render.device.create_command_encoder(&describe);
            self.draw_into(&mut encoder, &picture.view, as_parts);
            self.submit(
                render,
                encoder.finish(),
                (pixels, params.look, as_parts),
                params.quality,
            );
            self.drawn = Some(wanted);
        }
        let id = picture.id;
        self.picture = Some(picture);
        id
    }

    /// Hand a picture's drawing to the device. A picture in full is timed when it is the
    /// first after a coarser one, when none of its kind has been timed, and every so
    /// often while the moving scene is drawn in full: the device is waited for, which
    /// costs nothing the picture would not have cost anyway.
    fn submit(&mut self, render: &RenderState, work: wgpu::CommandBuffer, of: Timed, quality: f32) {
        let full = quality >= 1.0;
        let timing = &mut self.timing;
        let stale = timing.took.as_ref().is_none_or(|(timed, _)| *timed != of);
        let due = full && (stale || !timing.was_full || timing.untimed >= TIME_EVERY);
        timing.was_full = full;
        if !due {
            timing.untimed += u32::from(full);
            render.queue.submit([work]);
            return;
        }
        match timed_submit(&render.device, &render.queue, work) {
            Some(took) => {
                timing.took = Some((of, took));
                (timing.untimed, timing.tries) = (0, 0);
            }
            // The device was busy: this picture says nothing of how long one takes.
            None => timing.tries += u32::from(stale),
        }
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

    /// One render pass that clears the picture to black and draws the volume over it, or
    /// the labelled parts.
    fn draw_into(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        picture: &wgpu::TextureView,
        as_parts: bool,
    ) {
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
        if as_parts {
            self.parts.draw(&mut pass);
        } else {
            self.pipeline.draw(&mut pass);
        }
    }
}
