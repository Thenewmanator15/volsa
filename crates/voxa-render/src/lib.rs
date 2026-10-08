//! GPU drawing of image slices.
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

mod offscreen;
mod slice_pipeline;
mod view;

pub use offscreen::{Headless, render_to_rgba};
pub use slice_pipeline::{RenderError, SlicePipeline};
pub use view::{MAX_ZOOM, MIN_ZOOM, ViewTransform};
