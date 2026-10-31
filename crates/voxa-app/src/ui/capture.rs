//! Saving a picture of the window to a file, for checking by eye and for the documentation.
//!
//! Set the environment variable `VOXA_CAPTURE` to a `.png` path. Voxa then saves one picture
//! of its own window, once the first viewport has something to show or after a wait, and
//! closes. Only Voxa's window is in the picture, never the rest of the screen. With
//! `VOXA_CAPTURE_SIZE` set as well, to a size such as `900x600`, the window is that size, in
//! points of the interface as it is drawn. With `VOXA_CAPTURE_ZOOM` set to a number, the
//! interface is drawn at that size, 1 being its usual size: on a display of 1440 by 900 points
//! at 200%, 0.75 shows a window 1920 points across, as on a screen of 1920 by 1080 pixels at
//! 100%, and 0.375 shows one 3840 points across. A picture's window is not fitted to the
//! screen unless it asks to be.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use voxa_core::{SeriesId, Volume};

use super::VoxaApp;
use super::theme::ThemeChoice;
use super::viewport::Gpu;
use crate::action::Action;
use crate::labels::LabelsStatus;
use crate::scene::SceneStatus;
use crate::state::AppState;
use labelling::Run;

mod labelling;
mod measured;
mod motion;
mod poses;
mod series;
mod tabs;

pub use labelling::made_up_look;
pub use measured::sample_annotations;
pub use motion::{Motion, frames_before, turn_at};
pub use poses::{arrangement_asked, label_pose, pose};
pub use series::{asked_for, first_listed, listed_as};
pub use tabs::{opens, tab_asked};

/// The environment variable naming the file to save.
pub const VARIABLE: &str = "VOXA_CAPTURE";
/// The environment variable naming what to have in the picture, separated by commas. The words:
///
/// - Windows and lists open over the pictures: `tags` (the tags of a file), `keys` (the keys and
///   what the mouse does), `about`, `notices` (the list of notices above the bottom line, opened
///   once there is a notice to list; with none, the picture is taken after 20 seconds without it) and `search` (the search box).
/// - The tab open in the panel at the right: `tab-view`, `tab-measure`, `tab-3d`, `tab-labels`
///   or `tab-tools`; or, as earlier pictures ask, `measure` (the Measure tab), `labelling` (the
///   Tools tab, with the strip that labels a scan, as it is on a computer that can label) and
///   `parts` (the Labels tab).
/// - How the pictures are arranged: `planes` or `quad` for the three planes with the 3D view,
///   `3d` for the 3D view alone and `slices3d` for the slices beside it. With none of these,
///   the slices alone, as Voxa opens.
/// - The 3D scene: the looks `vr`, `mip`, `solids` (the labelled parts as solids) and the
///   poses `turned`, `cut`, `slice` (the slice drawn in it), `smooth` (smooth planes), `filled`
///   (labels filled over the planes), `faded` and `chosen` (the first part faded or selected),
///   and `moving` for a picture taken while the scene is kept moving.
/// - What is measured and shown: `measured` for one of each kind of annotation in the axial
///   plane, `first` for the first series listed in place of the one Voxa opens itself, `stepped`
///   for the next volume of a series that holds several, `labelled` for a result that Voxa
///   makes itself (the picture starts a run and waits for it).
/// - The window: `nolist` for the rail of scans folded however wide the window is, `other` for
///   the other files open in the rail, `light` for the light theme in place of the dark one,
///   and `fitted` for the window fitted to the screen as it is when Voxa starts with no size
///   remembered, in place of the size asked for.
pub const SHOW_VARIABLE: &str = "VOXA_CAPTURE_SHOW";
/// The environment variable giving words from the description of the series to show in
/// place of the one Voxa opens itself: the first listed whose description holds them.
pub const SERIES_VARIABLE: &str = "VOXA_CAPTURE_SERIES";
/// The environment variable giving the size of the window for the picture, in points of the
/// interface as it is drawn, as width `x` height: `900x600`.
pub const SIZE_VARIABLE: &str = "VOXA_CAPTURE_SIZE";
/// The environment variable giving the size the interface is drawn at for the picture, 1
/// being its usual size: `0.75` puts four points where there were three.
pub const ZOOM_VARIABLE: &str = "VOXA_CAPTURE_ZOOM";
/// Frames drawn with a picture showing before it is captured, so everything has settled.
const SETTLE_FRAMES: u32 = 30;
/// How long to wait for a picture before capturing whatever is there.
const PATIENCE: Duration = Duration::from_secs(20);

/// Where settings are kept for this run: the folder the system gives, unless the run is to
/// take a picture of the window. A picture shows nothing kept from another run, the saved
/// views and annotations of whoever uses this computer, and leaves nothing behind.
pub fn kept_between_runs(capturing: bool, folder: Option<PathBuf>) -> Option<PathBuf> {
    folder.filter(|_| !capturing)
}

/// The settings folder, for the preferences alone, and the folder on this computer alone, for
/// the log and the rest (see [`crate::moved`]), as [`kept_between_runs`] has them.
pub fn folders(capturing: bool) -> (Option<PathBuf>, Option<PathBuf>) {
    (
        kept_between_runs(capturing, crate::settings::folder()),
        kept_between_runs(capturing, crate::log::folder()),
    )
}

/// A window size written as width `x` height. `None` unless both are numbers above zero.
pub fn size_from(text: &str) -> Option<[f32; 2]> {
    let (width, height) = text.split_once('x')?;
    let side = |text: &str| text.trim().parse::<f32>().ok().filter(|side| *side > 0.0);
    Some([side(width)?, side(height)?])
}

/// An interface size written as a number. `None` unless it is a number above zero.
pub fn zoom_from(text: &str) -> Option<f32> {
    let size = text.trim().parse::<f32>().ok()?;
    (size.is_finite() && size > 0.0).then_some(size)
}

/// The size of a window, in points at the interface's usual size, that holds `size` points of
/// an interface drawn at `zoom`.
pub fn window_for(size: [f32; 2], zoom: f32) -> [f32; 2] {
    size.map(|side| side * zoom)
}

/// Whether `wanted`, words separated by commas, holds `word` as one of them.
pub fn asks_for(wanted: &str, word: &str) -> bool {
    wanted.split(',').any(|part| part.trim() == word)
}

/// Whether the picture asks for `word` in [`SHOW_VARIABLE`].
pub fn wants(word: &str) -> bool {
    std::env::var(SHOW_VARIABLE).is_ok_and(|wanted| asks_for(&wanted, word))
}

/// The size the interface is drawn at for a picture that asks for one.
pub fn zoom() -> Option<f32> {
    zoom_from(&std::env::var(ZOOM_VARIABLE).ok()?)
}

/// The theme a window is drawn in as it opens, when `wanted` is what a picture is asked to hold:
/// a picture is drawn in light if it asks for `light` and otherwise in dark, whatever the system
/// is set to, so that it is the same wherever it is made. A window that is not a picture is
/// drawn in `remembered`.
pub fn theme_for(wanted: &str, capturing: bool, remembered: ThemeChoice) -> ThemeChoice {
    match (capturing, asks_for(wanted, "light")) {
        (false, _) => remembered,
        (true, true) => ThemeChoice::Light,
        (true, false) => ThemeChoice::Dark,
    }
}

/// The theme the window is drawn in as it opens: see [`theme_for`].
pub fn theme(capturing: bool, remembered: ThemeChoice) -> ThemeChoice {
    let wanted = std::env::var(SHOW_VARIABLE).unwrap_or_default();
    theme_for(&wanted, capturing, remembered)
}

/// A capture that has been asked for and not yet finished.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is one more thing the picture is still waiting to have done"
)]
pub struct Capture {
    path: PathBuf,
    started: Instant,
    settled: u32,
    asked: bool,
    /// Whether the first series listed has still to be shown for the picture.
    first: bool,
    /// Words from the description of a series still to be shown for the picture.
    series: Option<String>,
    /// Whether one of each kind of annotation has still to be made for the picture.
    measured: bool,
    /// Whether the list of notices has still to be opened for the picture.
    notices: bool,
    /// Whether the picture is to be of the 3D scene at rest or while it moves.
    scene: Motion,
    /// The run of labelling the picture waits for, if it asks for one.
    run: Run,
}

impl Capture {
    /// A capture, if the environment asks for one.
    pub fn from_environment() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os(VARIABLE)?);
        let wanted = std::env::var(SHOW_VARIABLE).unwrap_or_default();
        let series = asked_for(std::env::var(SERIES_VARIABLE).ok());
        Some(Self::asking(path, &wanted, series))
    }

    /// A capture to be saved at `path` of what the words in `wanted` ask for, and of the series
    /// whose description holds `series`, if any.
    pub fn asking(path: PathBuf, wanted: &str, series: Option<String>) -> Self {
        let asks = |word: &str| asks_for(wanted, word);
        Self {
            path,
            started: Instant::now(),
            settled: 0,
            asked: false,
            first: asks("first"),
            series,
            measured: asks("measured"),
            notices: asks("notices"),
            scene: if asks("moving") {
                Motion::Moving
            } else {
                Motion::Still
            },
            run: Run::asked(wanted),
        }
    }

    /// Whether the list of notices is to be opened now for a picture that asks for it: once,
    /// as soon as there is a notice to list, which is not so as the window opens.
    pub fn open_notices(&mut self, has_notices: bool) -> bool {
        let now = self.notices && has_notices;
        self.notices &= !now;
        now
    }

    /// Whether the list of notices the picture asks for, if it does, has been opened.
    pub fn has_opened_notices(&self) -> bool {
        !self.notices
    }

    /// What shows the series the picture asks for, when it asks for one: given once.
    pub fn pick(&mut self, state: &AppState, has: impl Fn(&SeriesId) -> bool) -> Option<Action> {
        let pick = match &self.series {
            Some(words) => listed_as(state, words, has),
            None => first_listed(state, has).filter(|_| self.first),
        }?;
        self.first = false;
        self.series = None;
        Some(pick)
    }

    /// Whether the series the picture asks for, if any, has been shown.
    pub fn has_picked(&self) -> bool {
        !self.first && self.series.is_none()
    }

    /// The annotations the picture asks for, when it asks for them: given once, when the
    /// scene's volume is there to make them on.
    pub fn sample(&mut self, state: &AppState, scan: Option<&Volume>) -> Vec<Action> {
        let made = scan
            .filter(|_| self.measured)
            .map(|scan| sample_annotations(state, scan));
        let made = made.unwrap_or_default();
        self.measured &= made.is_empty();
        made
    }

    /// What starts the run of labelling the picture asks for, once labelling can run: given
    /// once.
    pub fn label(&mut self, state: &AppState) -> Option<Action> {
        self.run.start(state)
    }

    /// Whether the run of labelling the picture asks for, if any, is over.
    pub fn has_labelled(&self, state: &AppState) -> bool {
        self.run.is_over(state)
    }

    /// For a picture of the scene while it moves: how to turn it this frame, a little one
    /// way and then back, once it has had time to come to rest and be drawn in full.
    pub fn turn(&self) -> Option<Action> {
        turn_at(self.scene, self.settled)
    }

    /// Call once a frame. `ready` says whether the window shows what is wanted.
    pub fn step(&mut self, ctx: &egui::Context, ready: bool) {
        let taken = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(picture) = taken {
            let [width, height] = picture
                .size
                .map(|side| u32::try_from(side).unwrap_or(u32::MAX));
            let colour = image::ExtendedColorType::Rgba8;
            // Nothing can be said to anyone if saving fails: the window is about to close.
            let _ = image::save_buffer(&self.path, picture.as_raw(), width, height, colour);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if ready {
            self.settled += 1;
        }
        let frames = frames_before(self.scene);
        let patience = self.run.patience(PATIENCE);
        let due = self.settled >= frames || self.started.elapsed() >= patience;
        if due && !self.asked {
            self.asked = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }
}

impl VoxaApp {
    /// What a frame does for a picture of the window, if one has been asked for: pose the
    /// scene and make the annotations the picture wants, once the volume is ready; show the
    /// series it wants; and take the picture when everything asked for is on the screen.
    pub(super) fn for_a_picture(&mut self, ctx: &egui::Context) {
        // The scene is posed once it shows the series the picture asks for.
        let picked = self.capture.as_ref().is_none_or(Capture::has_picked);
        if picked && self.session.state().scene.status == SceneStatus::Ready {
            for action in std::mem::take(&mut self.pose) {
                self.session.dispatch(action);
            }
            let state = self.session.state();
            let shown = state.scene.series.as_ref();
            let scan = shown.and_then(|id| self.session.volume(id));
            let wanted = self.capture.as_mut();
            let sample = wanted.map(|capture| capture.sample(state, scan.as_deref()));
            for action in sample.unwrap_or_default() {
                self.session.dispatch(action);
            }
            if let Some(turn) = self.capture.as_ref().and_then(Capture::turn) {
                self.session.dispatch(turn);
            }
        }
        // A picture of a result that Voxa makes starts the run once it can, and waits for it.
        let state = self.session.state();
        let capture = self.capture.as_mut().filter(|_| picked);
        if let Some(start) = capture.and_then(|capture| capture.label(state)) {
            self.session.dispatch(start);
        }
        // A picture of the window waits for the slice and, if there is to be one, the volume.
        let no_volume_to_come = !matches!(
            self.session.state().scene.status,
            SceneStatus::Building { .. } | SceneStatus::Ready
        );
        let slices = self.slices_hidden() || self.gpus.first().is_some_and(Gpu::has_picture);
        let volume = self.volume_shown();
        // And for labels that are still to be opened, or are being read.
        let reading = self.session.state().labels.status == LabelsStatus::Reading;
        let waiting = self.labels.is_waiting() && !no_volume_to_come;
        // What the picture wants of the labels is done once they are read, a thing a frame:
        // the scene is drawn between one and the next, as it is when a person asks.
        let read = self.session.state().labels.status == LabelsStatus::Ready;
        if read && !self.label_pose.is_empty() {
            let next = self.label_pose.remove(0);
            self.session.dispatch(next);
        }
        let posed = self.label_pose.is_empty() || !(waiting || reading || read);
        let state = self.session.state();
        let ran = self
            .capture
            .as_ref()
            .is_none_or(|capture| capture.has_labelled(state));
        let labelled = !waiting && !reading && posed && ran;
        // A picture that asks for the list of notices opens it once there is a notice to list.
        let has_notices = !self.session.state().notices.is_empty();
        let open = self.capture.as_mut();
        if open.is_some_and(|capture| capture.open_notices(has_notices)) {
            self.bars.notices_open = true;
        }
        let listed = self
            .capture
            .as_ref()
            .is_none_or(Capture::has_opened_notices);
        let ready = slices && labelled && volume && listed;
        if let Some(capture) = &mut self.capture {
            let pick = capture.pick(self.session.state(), |id| self.thumbnails.contains_key(id));
            capture.step(ctx, ready && pick.is_none());
            if let Some(pick) = pick {
                self.session.dispatch(pick);
            }
        }
    }

    /// Whether a picture of the window has the volume in it as it should be: none is to come;
    /// or it is ready and, with the 3D view among the pictures, drawn there, and the planes,
    /// if they are shown and to be drawn smooth, have the curve they are drawn from.
    fn volume_shown(&self) -> bool {
        let state = self.session.state();
        match state.scene.status {
            SceneStatus::Building { .. } => false,
            SceneStatus::Ready => {
                let plain = !state.planes.smooth || !crate::planes::in_sight(state);
                let curve = plain || self.planes.is_smooth();
                curve && (!self.scene_in_sight() || self.scene.has_picture())
            }
            SceneStatus::Empty | SceneStatus::Unavailable(_) => true,
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod words_tests;
