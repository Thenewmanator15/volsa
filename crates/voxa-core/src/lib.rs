//! Data model, typed coordinate spaces and errors shared by every Voxa crate.
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

mod error;
mod grid;
mod held;
mod kind;
mod model;
mod rescale;
mod scan;
mod space;
mod volume;
mod window;

pub use error::VoxaError;
pub use grid::Grid;
pub use held::{HeldError, HeldVolume, SlabRaw, SlabStore, SliceIn, Stored};
pub use kind::{MOST_LOCATING, SeriesKind};
pub use model::{SeriesId, SeriesInfo, SeriesProblem, StudyId, StudyInfo, VolumeSet};
pub use rescale::Rescale;
pub use scan::{Facing, Family, Plane, ScanFacts};
pub use space::{Affine, Patient, Point, Screen, Voxel};
pub use volume::Volume;
pub use window::WindowLevel;
