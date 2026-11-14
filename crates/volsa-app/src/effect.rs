//! Work to be done outside the state.

use std::path::PathBuf;

use volsa_core::{SeriesId, StudyId};
use volsa_models::{RunsOn, Wanted};

use crate::annotations::Annotation;
use crate::choices::Choices;
use crate::places::Place;
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
    /// Read where the series of the export that is open were left, and which of them was last
    /// shown in the first viewport.
    LoadPlaces {
        /// Which export this is, as the state counts them: what is read is reported with it.
        opened: u64,
        /// Every series of the export.
        series: Vec<SeriesId>,
        /// Every study of the export.
        studies: Vec<StudyId>,
    },
    /// Write where series were left to the file the places are kept in.
    StorePlaces {
        /// The series left, each with its place.
        places: Vec<(SeriesId, Place)>,
        /// A study and the series of it now shown in the first viewport, when that is not
        /// the one kept already.
        first: Option<(StudyId, SeriesId)>,
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
    /// Highlight the thin dark tubes of a series' volume, and take them in as labels read.
    FindVeins {
        /// The series.
        series: SeriesId,
        /// Whether its description names it as SWI or T2*, on which most such tubes are veins.
        named: bool,
        /// Where the names of the labels open on its study came from, when the tubes are to be
        /// put onto them as one more part; `None` to open them alone.
        onto: Option<crate::labels::NamesFrom>,
    },
    /// Work out a susceptibility map from the phase volume kept for a series, keep it in that
    /// volume's place and say it is ready as a volume is.
    MakeSusceptibility {
        /// The phase series.
        series: SeriesId,
        /// Its echo time and field strength, from its files.
        facts: volsa_segment::PhaseFacts,
    },
    /// Give up the susceptibility map kept, and any being worked out.
    DropSusceptibility,
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
        /// How many bytes of the graphics card Volsa draws on it holds itself.
        held: u64,
        /// Whether to keep the result.
        keep: bool,
        /// The name of the card Volsa draws on, if it draws on one.
        drawing_on: Option<String>,
    },
    /// Stop the run of labelling, if one is on, and let go of labels a run made that the
    /// state has not taken.
    StopLabelling,
    /// Hold the labels that run `run` made as the labels kept, in place of any before, and keep
    /// them for their series if the run was to: the state has taken them in. Until then they
    /// are neither drawn nor saved nor kept.
    TakeLabels {
        /// Which run made them.
        run: u64,
    },
    /// Look for the result kept for a series, and report it if there is one.
    FindKept {
        /// The series.
        series: SeriesId,
    },
    /// Delete the result kept for a series.
    ForgetKept {
        /// The series.
        series: SeriesId,
    },
    /// Write the labels that are open to a file, and their table beside it.
    SaveLabels {
        /// The file for the labels, which is given the ending `.nii.gz` if it lacks it; the
        /// table goes beside it, under the same name with `.json`.
        to: PathBuf,
    },
    /// Keep what the person has chosen for labelling, for the next run of Volsa.
    StoreChoices(Choices),
    /// Something about the plugins the person added.
    Plugins(crate::plugins::PluginEffect),
    /// Read the structured report in an entry of the open source as lines of text.
    ReadDocument {
        /// The entry that holds the report's file.
        entry: String,
    },
    /// Write the PDF stored in an entry of the open source to the place the person chose.
    SaveDocumentPdf {
        /// The entry that holds the file the PDF is stored in.
        entry: String,
        /// Where to write the copy.
        to: PathBuf,
    },
    /// Nothing to do.
    None,
}
