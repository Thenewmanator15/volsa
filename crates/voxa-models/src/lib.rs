//! The networks Voxa labels a scan with. This crate finds a model folder, reads the list that
//! says what the folder should hold and checks every file against that list. It lists the
//! graphics cards of the computer. It works out, from what a run needs and what the computer
//! has, which devices the run fits on and in what order to try them. And it runs a network,
//! on the processor or on a card, through ONNX Runtime loaded from a folder. It never fetches
//! a network or the runtime: both folders are the person's own.
// `unsafe` is allowed in one module, `cards`: the system's list of graphics cards has no
// other door. Running a network needs none.
#![deny(unsafe_code)]
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

#[allow(unsafe_code)]
mod cards;
mod model;
mod network;
mod room;
mod runtime;
mod tensor;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use cards::{
    Card, CardId, CardKind, FreeMemory, budget, cards, free_memory, in_use, place_of, total_memory,
    unheld,
};
pub use model::{
    KIND_SYNTHSEG, Lists, MODEL_FORMAT, Model, ModelError, ModelSide, Role, Structure, read_model,
};
pub use network::{Device, Network, RunError, Stop};
pub use room::{Cut, Room, Rung, RunsOn, Wanted, asks, needed_gb, needs, rungs, spare_gb, why_not};
pub use runtime::{Runtime, RuntimeError};
pub use tensor::{Numbers, Tensor};
