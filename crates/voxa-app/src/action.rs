//! Everything that can happen to the application, and the work it can ask for.

use std::path::PathBuf;

use voxa_core::{SeriesId, WindowLevel};
use voxa_dicom::Index;

use crate::state::{Layout, PresetWindows};

/// One thing that happened: something the person did, or something a worker finished.
/// [`reduce`](crate::reduce) is the only code that acts on these.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Open a folder, zip or file in place of whatever is open.
    OpenPath(PathBuf),
    /// The index of the source so far.
    IndexProgress(Index),
    /// The source could not be opened or holds no images; the message says which.
    IndexFailed(String),
    /// Indexing has finished.
    IndexDone,
    /// Show a series in a viewport.
    SelectSeries {
        /// Which viewport.
        viewport: usize,
        /// Which series.
        series: SeriesId,
    },
    /// Move through the slices by `delta`, stopping at either end.
    Scroll {
        /// Which viewport.
        viewport: usize,
        /// How many slices, forwards or backwards.
        delta: i32,
    },
    /// Go to a slice, stopping at the last.
    SetSlice {
        /// Which viewport.
        viewport: usize,
        /// Which slice, counting from 0.
        slice: usize,
    },
    /// Magnify by `factor`, keeping still what lies under `about`.
    Zoom {
        /// Which viewport.
        viewport: usize,
        /// How many times larger; below 1 shrinks.
        factor: f32,
        /// The point to keep still, in screen pixels from the middle of the viewport.
        about: [f32; 2],
    },
    /// Shift the picture.
    Pan {
        /// Which viewport.
        viewport: usize,
        /// How far, in screen pixels.
        by: [f32; 2],
    },
    /// Mirror the picture as it is seen.
    Flip {
        /// Which viewport.
        viewport: usize,
        /// Left to right when true, otherwise top to bottom.
        horizontal: bool,
    },
    /// Turn the picture a quarter turn clockwise.
    Rotate {
        /// Which viewport.
        viewport: usize,
    },
    /// Show the whole picture again, undoing zoom and pan.
    Fit {
        /// Which viewport.
        viewport: usize,
    },
    /// Change the window by dragging.
    AdjustWindow {
        /// Which viewport.
        viewport: usize,
        /// Added to the centre.
        d_center: f32,
        /// Added to the width.
        d_width: f32,
    },
    /// Set the window outright.
    SetWindow {
        /// Which viewport.
        viewport: usize,
        /// The window wanted.
        wl: WindowLevel,
    },
    /// Use one of [`voxa_volume::PRESETS`] on the slice shown.
    ApplyPreset {
        /// Which viewport.
        viewport: usize,
        /// Which preset, by its place in the list.
        preset: usize,
    },
    /// Arrange the viewports.
    SetLayout(Layout),
    /// Make a viewport the one keyboard commands act on.
    SetActive(usize),
    /// Switch between radiological and neurological display.
    ToggleRadiological,
    /// Show or hide the patient's name and identifier.
    TogglePatient,
    /// Filter the tag browser.
    SetTagFilter(String),
    /// Start or pause film-like playback.
    CineToggle {
        /// Which viewport.
        viewport: usize,
    },
    /// Set the speed of playback and whether it loops.
    CineSet {
        /// Which viewport.
        viewport: usize,
        /// Slices each second.
        fps: f32,
        /// Whether to start again after the last slice.
        looping: bool,
    },
    /// Playback is due to show the next slice.
    CineTick {
        /// Which viewport.
        viewport: usize,
    },
    /// A slice has been decoded and is in the frame cache.
    FrameReady {
        /// The series it belongs to.
        series: SeriesId,
        /// Which slice.
        slice: usize,
        /// The window each preset gives for it.
        windows: PresetWindows,
    },
    /// A slice could not be decoded.
    FrameFailed {
        /// The series it belongs to.
        series: SeriesId,
        /// Which slice.
        slice: usize,
        /// Why, in plain words.
        reason: String,
    },
    /// Remove a notice, by its place in the list.
    DismissNotice(usize),
}

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
    /// Nothing to do.
    None,
}
