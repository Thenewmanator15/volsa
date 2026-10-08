//! Reading scan exports: sources, header indexing and pixel decoding.
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

/// Name of this crate, as shown in logs and the About window.
pub const CRATE_NAME: &str = "voxa-dicom";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {}
}
