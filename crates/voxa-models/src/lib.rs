//! The networks Voxa labels a scan with. This crate finds a model folder, reads the list that
//! says what the folder should hold and checks every file against that list. It never runs
//! a network, and it never fetches one: the folder is the person's own.
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

mod model;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use model::{
    KIND_SYNTHSEG, Lists, MODEL_FORMAT, Model, ModelError, ModelSide, Role, Structure, read_model,
};
