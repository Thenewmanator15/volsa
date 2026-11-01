//! The scene on the graphics device: the volume, the off-screen picture it is drawn into,
//! and that picture as the interface shows it.

use std::sync::{Arc, Weak};
use std::time::Duration;

use egui_wgpu::{RenderState, wgpu};
use volsa_core::{Grid, HeldVolume, Volume};
use volsa_render::{
    CardRoom, LabelsOnDevice, Look, PICTURE_FORMAT, PartsPipeline, RenderError, SceneParams,
    UPLOAD_BYTES_A_FRAME, UploadState, VolumePipeline, timed_submit,
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
    given: Weak<HeldVolume>,
    drawn: Option<Drawn>,
    timing: Timing,
    /// How often the volume being handed over was halved to fit.
    halved: u32,
    /// A preview of the series being built, drawn until its volume is on the device, and
    /// made only when there is one.
    preview: Option<VolumePipeline>,
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
            halved: 0,
            preview: None,
        }
    }

    /// Whether this volume is the one the graphics device was last given.
    pub fn holds(&self, volume: &Arc<HeldVolume>) -> bool {
        Weak::ptr_eq(&self.given, &Arc::downgrade(volume))
    }

    /// Whether there is a volume on the graphics device to draw, or a preview of one.
    pub fn has_volume(&self) -> bool {
        self.pipeline.has_volume() || self.has_preview()
    }

    fn has_preview(&self) -> bool {
        self.preview
            .as_ref()
            .is_some_and(VolumePipeline::has_volume)
    }

    /// Whether a volume is on its way to the graphics device.
    pub fn is_uploading(&self) -> bool {
        self.pipeline.is_uploading()
    }

    /// Draw a preview of the series being built: the one `offered`, if a new one is, or the
    /// one given before. Whether there is one to draw.
    pub fn preview(&mut self, render: &RenderState, offered: Option<Volume>) -> bool {
        if let Some(preview) = offered {
            let device = &render.device;
            let pipeline = self
                .preview
                .get_or_insert_with(|| VolumePipeline::new(device, FORMAT));
            if pipeline.upload(device, &render.queue, &preview).is_ok() {
                self.drawn = None;
            }
        }
        self.has_preview()
    }

    /// Give up a volume on its way to the device, and the volume itself with it: its series
    /// is no longer the one shown.
    pub fn give_up_upload(&mut self) {
        if self.pipeline.is_uploading() {
            self.pipeline.give_up_upload();
            self.given = Weak::new();
        }
    }

    /// Give up the preview, if there is one.
    pub fn drop_preview(&mut self) {
        if self.preview.take().is_some() {
            self.drawn = None;
        }
    }

    /// What is drawn: the preview while there is one, and the volume otherwise.
    fn shown(&self) -> &VolumePipeline {
        self.preview
            .as_ref()
            .filter(|preview| preview.has_volume())
            .unwrap_or(&self.pipeline)
    }

    /// The pipeline that holds the volume.
    pub fn volume_pipeline(&self) -> &VolumePipeline {
        &self.pipeline
    }

    /// Draw the Parts look from these labels, or have none to draw from. `stamp` moves on
    /// whenever the labels, or how their parts look, change.
    pub fn label(&mut self, render: &RenderState, with: Option<&LabelsOnDevice>, stamp: u64) {
        self.parts.show(&render.device, &render.queue, with);
        self.labels = stamp;
    }

    /// Whether a scene is drawn as labelled solids: that is its look, and there are labels.
    fn as_parts(&self, params: &SceneParams) -> bool {
        params.look == Look::Parts && self.parts.has_labels()
    }

    /// Hand a volume to the graphics device, a slab a frame, unless it is the one given
    /// already. `card` is what the device is to hold of it, the volume or a copy halved to
    /// fit, with how often it was halved, and what the device can hold. Asks for another
    /// frame while the volume is on its way, and says how many times it was halved once the
    /// whole of it is there. A volume the device has no room for as it is is put there
    /// halved as often as it takes, here and now; if there is no room for it at all,
    /// nothing is left there to draw, and it is not tried again.
    pub fn hold(
        &mut self,
        render: &RenderState,
        volume: &Arc<HeldVolume>,
        (card, room): (Option<(Arc<HeldVolume>, u32)>, CardRoom),
        ctx: &egui::Context,
    ) -> Option<u32> {
        let (device, queue) = (&render.device, &render.queue);
        if !self.holds(volume) {
            let (held, halved) = card.unwrap_or_else(|| (Arc::clone(volume), 0));
            self.given = Arc::downgrade(volume);
            self.drawn = None;
            self.timing = Timing::default();
            self.halved = halved;
            if self.pipeline.start_upload(device, held, room).is_err() {
                return self.halved_here(render, volume);
            }
        }
        match self
            .pipeline
            .continue_upload(device, queue, UPLOAD_BYTES_A_FRAME)
        {
            UploadState::Going { .. } => {
                ctx.request_repaint();
                None
            }
            UploadState::Done => {
                // The volume itself is drawn from now on.
                self.preview = None;
                self.drawn = None;
                Some(self.halved)
            }
            UploadState::Failed(RenderError::NoRoom) => self.halved_here(render, volume),
            UploadState::Failed(_) | UploadState::Idle => None,
        }
    }

    /// Put a volume on the device halved as often as it takes, from its values made here:
    /// what is done only when the device turned down what it was offered.
    fn halved_here(&mut self, render: &RenderState, volume: &Arc<HeldVolume>) -> Option<u32> {
        let values = volume.to_volume(0..volume.dims()[2])?;
        self.pipeline
            .upload(&render.device, &render.queue, &values)
            .ok()
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
            } else if let Some(preview) = self.preview.as_mut().filter(|it| it.has_volume()) {
                preview.set_params(&render.queue, params, viewport);
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
            label: Some("volsa scene picture"),
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
            label: Some("volsa scene"),
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
            self.shown().draw(&mut pass);
        }
    }
}
