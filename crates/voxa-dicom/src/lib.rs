//! Reading scan exports: sources, header indexing and pixel decoding.
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

mod source;

pub use source::{Entry, SeriesSource, open_source};
pub use voxa_core::VoxaError;
