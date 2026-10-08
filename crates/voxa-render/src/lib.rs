//! GPU drawing of image slices.
#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)] // any `unsafe` here carries a SAFETY comment
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

/// Name of this crate, as shown in logs and the About window.
pub const CRATE_NAME: &str = "voxa-render";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {}
}
