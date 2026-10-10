//! Labelled parts from files: label volumes made elsewhere, the tables that name their
//! parts, and the volume of each part. Voxa shows what such a file holds; it does not make
//! the labels.
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

mod nifti;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use nifti::{LabelGrid, MAX_LABEL_VOXELS, NiftiError, read_labels};
