//! Image statistics: histograms and automatic window/level.
#![forbid(unsafe_code)]
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

mod auto_window;
mod histogram;

pub use auto_window::{PRESETS, Preset, auto_window};
pub use histogram::Histogram;
