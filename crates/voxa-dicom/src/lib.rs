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
#![cfg_attr(test, allow(clippy::float_cmp))] // tests compare against exactly representable values

mod decode;
mod document;
mod geometry;
mod header;
mod index;
mod nesting;
mod source;
mod tags;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use decode::{Frame, ImageFile, StoredFrame, StoredSamples, decode_slice};
pub use document::{
    DocumentKind, IndexedDocument, MAX_REPORT_CHARS, MAX_REPORT_DEPTH, MAX_REPORT_LINES, Report,
    ReportLine, pdf_bytes, read_report,
};
pub use geometry::{StackGeometry, stack_geometry};
pub use index::{Index, IndexedSeries, SliceRef, best_series, index};
pub use nesting::{MAX_NESTING, READING_STACK_BYTES, Refused};
pub use source::{Entry, SeriesSource, open_source};
pub use tags::{TagRow, is_patient_identifying, list_tags};
pub use voxa_core::VoxaError;
