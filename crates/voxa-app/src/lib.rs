//! The Voxa desktop application: interface, state and background work.
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

mod action;
mod annotations;
mod args;
mod cache;
mod card;
mod choices;
mod csv;
mod device;
mod effect;
mod export;
mod kept_labels;
mod labelling;
mod labels;
mod log;
mod measurements;
mod moved;
mod parts_table;
mod planes;
mod prefs;
mod reducer;
mod said;
mod scene;
mod session;
mod settings;
mod state;
#[cfg(test)]
mod testing;
mod ui;
mod views;
mod volume_store;
mod worker;

pub use action::{Action, Effect};
pub use annotations::{
    ANNOTATIONS_VERSION, Annotation, Annotations, MAX_ANNOTATIONS, MAX_HISTORY, MAX_LABEL,
    Measured, Shape, Tool, load_annotations, measured, save_annotations,
};
pub use args::{Command, USAGE, parse_args, version};
pub use cache::FrameCache;
pub use choices::{CHOICES_VERSION, Choices, RunsOnKept, load_choices, save_choices};
pub use export::{ExportError, SUGGESTED_NAME, gif, png};
pub use labelling::{Labelling, LabellingAction, Ready, can_label, what_will_run};
pub use labels::{
    Labels, LabelsStatus, NOWHERE_NEAR, NamesFrom, PartShown, is_label_file, look_drawn,
    on_this_series, study_of,
};
pub use measurements::{MEASUREMENTS_NAME, Row, measurements_csv, measurements_json, rows};
pub use planes::{ColumnShows, PlanePanel, Planes};
pub use reducer::reduce;
pub use said::{Outcome, WINDOWED, outcome, tell};
pub use scene::{MainPanel, Scene, SceneStatus};
pub use session::Session;
pub use state::{
    AppState, Cine, Layout, Notice, NoticeKind, PresetWindows, VIEWPORTS, ViewportState,
};
pub use ui::{VoxaApp, run, thumbnail};
pub use views::{MAX_VIEWS, SavedView, ViewsAction, series_key};
pub use volume_store::VolumeStore;
pub use worker::labelling::Labeller;
pub use worker::{MAX_LABEL_FILE_BYTES, NAMES_NOT_READ, Workers, table_beside};
