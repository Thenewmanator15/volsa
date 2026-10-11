//! Labelled parts from files: label volumes, the tables that name their parts, and the
//! volume of each part. This crate reads them, from files made elsewhere or by Voxa, and
//! writes them; it does not make the labels.
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
mod made;
mod nifti;
mod pairs;
mod set;
mod table;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
mod words;
mod write;

pub use find::{boxes_meet, first_part};
pub use made::{MAX_MADE_LINE, MAX_NOTE, MAX_NOTES, Made};
pub use nifti::{LabelGrid, MAX_LABEL_VOXELS, NiftiError, read_labels};
pub use pairs::{Pair, difference_percent, pairs};
pub use set::{LabelSet, NOT_NAMED, Part, SetError, label_set};
pub use table::{Entry, MAX_NAME, Side, Table, TableError, Trust, Verdict, colour_for, read_table};
pub use words::{trust_in_words, verdict_in_words, volume_in_words};
pub use write::{WriteError, write_labels, write_table};

#[cfg(test)]
mod damage_tests;
