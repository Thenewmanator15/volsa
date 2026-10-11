//! Everything `SynthSeg` does around its networks, step for step as `SynthSeg`'s own code does
//! it. A network is only as right as what it is given: this crate turns one of Voxa's volumes
//! into the image the networks were trained on, and remembers how to come back from it. It
//! takes the steps between the networks: what each is given, and what is made of its answer
//! before the next one runs. And it tidies what the networks found into a label set on the
//! volume's own grid, with the table that names its parts and the model's verdict on its own
//! work. The networks themselves are run by `voxa-models`.
//!
//! Each step is held against arrays that `SynthSeg`'s own Python made from small made-up
//! volumes (`tools/golden` makes them; they are under `tests/golden`).
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
mod pieces;
mod prepare;
mod table;
#[cfg(feature = "testing")]
pub mod testing;
mod tidy;

pub use find::{Found, Networks, Stage, find};
pub use label::{LabelError, Labelled, ModelChoice, THICK_MM, choose, label};
pub use pieces::largest_piece;
pub use prepare::{Back, PrepareError, Prepared, Sizes, prepare, resampled, sizes};
pub use table::{FAIL_BELOW, table};
pub use tidy::tidy;
