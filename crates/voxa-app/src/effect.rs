//! Work to be done outside the state.

use std::path::PathBuf;

use voxa_core::SeriesId;
use voxa_models::{RunsOn, Wanted};

use crate::annotations::Annotation;
use crate::views::SavedView;

/// Work to be done outside the state: [`reduce`](crate::reduce) asks, the workers do.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Open this path and index what it holds.
    StartIndex(PathBuf),
    /// Decode the slices of a series, those nearest `around` first.
    RequestFrames {
        /// The series wanted.
        series: SeriesId,
        /// The slice being shown.
        around: usize,
    },
    /// Decode the middle slice of every series, for the pictures in the browser.
    RequestThumbnails,
    /// Read the views kept for a series from before.
    LoadViews {
        /// The series wanted.
        series: SeriesId,
    },
    /// Write the views of a series to the file they are kept in.
    StoreViews {
        /// The series they are of.
        series: SeriesId,
        /// All of its views, oldest first.
        views: Vec<SavedView>,
    },
    /// Read the annotations kept for a series.
    LoadAnnotations {
        /// The series.
        series: SeriesId,
    },
    /// Keep the annotations of a series, in place of any kept for it before.
    StoreAnnotations {
        /// The series.
        series: SeriesId,
        /// Its annotations, oldest first.
        items: Vec<Annotation>,
    },
    /// Decode every slice of a series and stack them into a volume for the 3D scene.
    BuildVolume {
        /// The series wanted.
        series: SeriesId,
    },
    /// Work out the curve through the volume kept for a series, for planes drawn smooth.
    BuildSmooth {
        /// The series whose volume it is to run through.
        series: SeriesId,
    },
    /// Give up the curve, and any being worked out.
    DropSmooth,
    /// Read a file of labels, with the table that names its parts if one was chosen.
    ReadLabels {
        /// The label volume.
        labels: PathBuf,
        /// The table of names; `None` to look for one beside the labels.
        table: Option<PathBuf>,
    },
    /// Give up the labels kept, and any being read.
    DropLabels,
    /// Look for the models and the runtime, in these folders or where they are usually kept,
    /// and for the graphics cards, and report what was found.
    LookForLabelling {
        /// Which look this is: what is found is reported with it.
        look: u64,
        /// The folder of the models, when the person chose one.
        models: Option<PathBuf>,
        /// The folder of the runtime, when the person chose one.
        runtime: Option<PathBuf>,
    },
    /// Label a series with the networks.
    LabelScan {
        /// Which run this is: what it says is reported with it.
        run: u64,
        /// The series to label.
        series: SeriesId,
        /// Which networks, and whether the cortex is divided into its areas.
        wanted: Wanted,
        /// Where the person wants the run to go.
        runs_on: RunsOn,
        /// How many bytes of the graphics card Voxa draws on it holds itself.
        held: u64,
        /// Whether to keep the result.
        keep: bool,
        /// The name of the card Voxa draws on, if it draws on one.
        drawing_on: Option<String>,
    },
    /// Stop the run of labelling, if one is on.
    StopLabelling,
    /// Nothing to do.
    None,
}
