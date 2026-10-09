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
mod offscreen;
mod offscreen_scene;
mod pick;
mod scene;
mod slice_pipeline;
#[cfg(test)]
mod testing;
mod vector;
mod view;
mod volume_pipeline;
mod volume_texture;

pub use camera::{Camera, MAX_MM_PER_PX, MIN_MM_PER_PX, NamedView, ORBIT_DEGREES_PER_PX};
pub use offscreen::{Headless, render_to_rgba};
pub use offscreen_scene::HeadlessScene;
pub use pick::{pick, surface_threshold};
pub use scene::{CutPlane, Look, MAX_CUTS, SceneParams, SlicePlane};
pub use slice_pipeline::{RenderError, SlicePipeline};
pub use view::{MAX_ZOOM, MIN_ZOOM, ViewTransform};
pub use volume_pipeline::VolumePipeline;
