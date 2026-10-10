//! GPU drawing of image slices, and the camera a volume is seen from.
#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)] // any `unsafe` here carries a SAFETY comment
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]
#![cfg_attr(test, allow(clippy::float_cmp))] // tests compare against exactly representable values

mod camera;
mod gpu;
mod handle;
mod labels;
mod offscreen;
mod offscreen_scene;
mod on_device;
mod parts_pipeline;
mod pick;
mod plane_pipeline;
mod plane_view;
mod prepared;
mod scene;
mod section;
mod slice_pipeline;
mod smoothed;
#[cfg(test)]
mod testing;
mod timing;
mod vector;
mod view;
mod volume_params;
mod volume_pipeline;
mod volume_texture;

pub use camera::{
    Camera, MAX_MM_PER_PX, MIN_MM_PER_PX, NamedView, ORBIT_DEGREES_PER_PX, turntable,
};
pub use handle::{Handle, cut_handle, drag_to_mm, on_screen, slice_drag_to_scroll, slice_outline};
pub use labels::{FILL, LabelsOnDevice, Overlay, PartLook};
pub use offscreen::{Headless, render_to_rgba};
pub use offscreen_scene::HeadlessScene;
pub use parts_pipeline::PartsPipeline;
pub use pick::{pick, pick_part, surface_threshold};
pub use plane_pipeline::PlanePipeline;
pub use plane_view::{MAX_PLANE_ZOOM, MIN_PLANE_ZOOM, PlaneView};
pub use prepared::{MAX_VOLUME_BYTES, Prepared};
pub use scene::{CutPlane, Look, MAX_CUTS, SceneParams, SlicePlane};
pub use section::{box_section, position_cross};
pub use slice_pipeline::{RenderError, SlicePipeline};
pub use smoothed::{Smoothed, SmoothedTexture};
pub use timing::timed_submit;
pub use view::{MAX_ZOOM, MIN_ZOOM, ViewTransform};
pub use volume_pipeline::{PICTURE_FORMAT, VolumePipeline};
