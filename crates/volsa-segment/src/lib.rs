//! Everything `SynthSeg` does around its networks, step for step as `SynthSeg`'s own code does
//! it. A network is only as right as what it is given: this crate turns one of Volsa's volumes
//! into the image the networks were trained on, and remembers how to come back from it. It
//! takes the steps between the networks: what each is given, and what is made of its answer
//! before the next one runs. And it tidies what the networks found into a label set on the
//! volume's own grid, with the table that names its parts and the model's verdict on its own
//! work. The networks themselves are run by `volsa-models`.
//!
//! Each step is held against arrays that `SynthSeg`'s own Python made from small made-up
//! volumes (`tools/golden` makes them; they are under `tests/golden`).
//!
//! # Where the code comes from
//!
//! This crate is a port to Rust of `SynthSeg`'s own code: the steps of `predict_synthseg.py`
//! and the `lab2im` functions they call.
//!
//! `SynthSeg`, Copyright 2020 Benjamin Billot, is licensed under the Apache License, Version
//! 2.0, and is at <https://github.com/BBillot/SynthSeg>. The port changes it as
//! `docs/models.md` describes under what Volsa does differently from `SynthSeg`'s own program:
//! a part's volume is counted in voxels, the scan may be cut to the head when memory is short,
//! and the cortex network's answer is reduced to areas inside the file. The `NOTICE` file at
//! the root of the repository carries the same attribution.
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
mod label;
#[cfg(test)]
mod notice_tests;
mod pieces;
mod prepare;
mod table;
#[cfg(feature = "testing")]
pub mod testing;
mod tidy;
mod toward;
mod veins;

pub use find::{Found, Networks, Stage, find};
pub use label::{LabelError, Labelled, ModelChoice, THICK_MM, choose, label};
pub use pieces::largest_piece;
pub use prepare::{Back, PrepareError, Prepared, Sizes, Turn, fits, prepare, resampled, sizes};
pub use table::{FAIL_BELOW, table};
pub use tidy::tidy;
pub use toward::Toward;
pub use veins::{
    VEINS_COLOUR, VEINS_GROUP, VEINS_HOW, VEINS_PART, VEINS_TITLE, VeinsError, veins, veins_onto,
    veins_table,
};
