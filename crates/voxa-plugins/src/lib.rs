//! Models people add to Voxa themselves. A plugin is a folder holding an ONNX network and a
//! description, `plugin.json`, that says what scans it accepts, how a scan is prepared for it,
//! what it gives back, where it came from and under what licence. This crate reads and checks
//! such a folder, prepares a scan by the description's settings, and makes the network's
//! answer a label set. Voxa ships no plugin, fetches none and recommends none: a plugin is the
//! person's own, and everything made with one says so.
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

mod describe;
mod folder;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use describe::{
    Accepts, AlignOutput, Contrast, Contrasts, DescribeError, Description, FieldUnit, Fit, Gives,
    Input, Kind, LabelOutput, Layout, Listed, Moves, Name, NumberOutput, Output, Outside, Over,
    PLUGIN_FORMAT, Scaling, Side, ValueOutput, Voxels, read_description,
};
pub use folder::{DESCRIPTION, Offered, PluginError, check_folder};
