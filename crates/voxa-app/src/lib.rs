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
mod cache;
mod reducer;
mod state;
#[cfg(test)]
mod testing;
mod ui;
mod worker;

pub use action::{Action, Effect};
pub use cache::FrameCache;
pub use reducer::reduce;
pub use state::{
    AppState, Cine, Layout, Notice, NoticeKind, PresetWindows, VIEWPORTS, ViewportState,
};
pub use ui::{VoxaApp, run, thumbnail};
pub use worker::Workers;
