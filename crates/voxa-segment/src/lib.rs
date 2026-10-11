//! Everything `SynthSeg` does around its networks, step for step as `SynthSeg`'s own code does
//! it. A network is only as right as what it is given: this crate turns one of Voxa's volumes
//! into the image the networks were trained on, and remembers how to come back from it. The
//! networks themselves are run by `voxa-models`.
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

mod prepare;

pub use prepare::{Back, PrepareError, Prepared, Sizes, prepare, resampled, sizes};
