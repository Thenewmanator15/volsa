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

mod find;
#[cfg(test)]
mod find_tests;
mod nifti;
mod pairs;
mod set;
mod table;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
mod words;

pub use find::{boxes_meet, first_part};
pub use nifti::{LabelGrid, MAX_LABEL_VOXELS, NiftiError, read_labels};
pub use pairs::{Pair, difference_percent, pairs};
pub use set::{LabelSet, NOT_NAMED, Part, SetError, label_set};
pub use table::{Entry, MAX_NAME, Side, Table, TableError, Trust, Verdict, read_table};
pub use words::{trust_in_words, verdict_in_words, volume_in_words};

#[cfg(test)]
mod damage_tests;
