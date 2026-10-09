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
mod args;
mod cache;
mod export;
mod reducer;
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
pub use args::{Command, USAGE, parse_args, version};
pub use cache::FrameCache;
pub use export::{ExportError, SUGGESTED_NAME, gif, png};
pub use reducer::reduce;
pub use scene::{MainPanel, Scene, SceneStatus};
pub use session::Session;
pub use state::{
    AppState, Cine, Layout, Notice, NoticeKind, PresetWindows, VIEWPORTS, ViewportState,
};
pub use ui::{VoxaApp, run, thumbnail};
pub use views::{MAX_VIEWS, SavedView, series_key};
pub use volume_store::VolumeStore;
pub use worker::Workers;
