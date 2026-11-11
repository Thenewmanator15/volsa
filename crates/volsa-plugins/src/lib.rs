//! Models people add to Volsa themselves. A plugin is a folder holding an ONNX network and a
//! description, `plugin.json`, that says what scans it accepts, how a scan is prepared for it,
//! what it gives back, where it came from and under what licence. This crate reads and checks
//! such a folder, prepares a scan by the description's settings, and makes the network's
//! answer a label set. Volsa ships no plugin, fetches none and recommends none: a plugin is the
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

mod accepts;
mod blocks;
mod describe;
mod finish;
mod fit;
mod folder;
mod grid;
pub mod helper;
mod label_map;
mod measure;
mod onnx;
mod run;
mod scaling;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
mod watched;

pub use accepts::{Decision, contrasts_named, decide, spacing_note};
pub use blocks::BlockPlan;
pub use describe::{
    Accepts, AlignOutput, Contrast, Contrasts, DescribeError, Description, FieldUnit, Fit, Gives,
    Input, Kind, LabelOutput, Layout, Listed, Moves, Name, NumberOutput, Output, Outside, Over,
    PLUGIN_FORMAT, Scaling, Side, ValueOutput, Voxels, read_description,
};
pub use finish::{FinishError, labels_of};
pub use fit::{Wiring, fit};
pub use folder::{DESCRIPTION, Offered, PluginError, check_folder};
pub use grid::{Grid, Region, label_grid, overhang_mm, sample};
pub use label_map::LabelMap;
pub use measure::{
    CardFigures, Fits, HelperPipes, HelperStarter, MEASURED, Measured, ProcessStarter, measure,
    own_bytes, places, read_measured, write_measured,
};
pub use onnx::{Dim, Element, NetworkError, NetworkFacts, Port, read_network};
pub use run::{
    BLOCKS_NOTE, Installed, PluginRunError, Ran, Stage, installed, measuring_job, run_plugin,
};
pub use scaling::{Scale, ScaleError, Statistics, gather, outside_value, percentile, voxels_of};
