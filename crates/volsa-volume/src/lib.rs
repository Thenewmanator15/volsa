//! Whole series as volumes, and image statistics: assembling, sampling, rays, reduction,
//! histograms and automatic window/level.
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

mod assemble;
mod auto_window;
mod bspline;
#[cfg(test)]
mod held_tests;
mod histogram;
mod measure;
mod plane;
mod preview;
mod ray;
mod reduce;
mod sample;
#[cfg(test)]
mod testing;

pub use assemble::{VolumeError, assemble, can_be_volume, value_range};
pub use auto_window::{PRESETS, Preset, auto_window};
pub use bspline::{coefficients, sample_cubic};
pub use histogram::Histogram;
pub use measure::{RegionStats, angle_degrees, distance_mm, ellipse_stats};
pub use plane::{
    Plane, PlaneKind, central_voxel, clamped_into, nearest_slice, pitch_along, plane_of,
    position_on_slice, reslice, rises_along, scrolled,
};
pub use preview::{preview, preview_of_reduced, reduced};
pub use ray::{Ray, enter_and_leave, first_hit};
pub use reduce::{block_maxima, halved, halved_held, halvings_needed};
pub use sample::sample;
