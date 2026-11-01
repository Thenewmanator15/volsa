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
mod budget;
mod cache;
mod card;
mod choices;
mod csv;
mod device;
mod documents;
mod effect;
mod export;
mod kept_labels;
mod labelling;
mod labels;
mod log;
mod measurements;
mod measuring;
mod moved;
mod naming;
mod parts_table;
mod people;
mod pictures;
mod planes;
mod prefs;
mod profile;
mod reducer;
mod said;
mod scene;
mod session;
mod settings;
mod spill;
mod state;
#[cfg(test)]
mod testing;
mod tools;
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
pub use measuring::Scale;
pub use naming::{Group, group_of, plain_name};
pub use pictures::Pictures;
pub use planes::{PlanePanel, Planes};
pub use profile::{CT_WINDOWS, NamedWindow, Profile, Unit, readout, unit_of};
pub use reducer::reduce;
pub use said::{Outcome, WINDOWED, outcome, tell};
pub use scene::{Scene, SceneStatus};
pub use session::Session;
pub use state::{
    AppState, Cine, Layout, Notice, NoticeKind, PresetWindows, VIEWPORTS, ViewportState,
};
pub use tools::{ADDED_BY_YOU, MadeBy, ToolInfo};
pub use ui::{BareLayout, BareWindow, VoxaApp, run, thumbnail};
pub use views::{MAX_VIEWS, SavedView, ViewsAction, series_key};
pub use volume_store::VolumeStore;
pub use worker::labelling::Labeller;
pub use worker::{MAX_LABEL_FILE_BYTES, NAMES_NOT_READ, Workers, table_beside};
