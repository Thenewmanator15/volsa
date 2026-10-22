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
use voxa_render::Look;

use super::VoxaApp;
use super::viewport::Gpu;
use crate::action::Action;
use crate::labels::LabelsStatus;
use crate::planes::ColumnShows;
use crate::scene::MainPanel;
use crate::scene::SceneStatus;
use crate::state::AppState;
use labelling::Run;

mod labelling;
mod measured;

pub use labelling::made_up_look;
pub use measured::sample_annotations;

/// The environment variable naming the file to save.
pub const VARIABLE: &str = "VOXA_CAPTURE";
/// The environment variable naming what to have in the picture, separated by commas: the
/// windows `tags`, `keys` and `about`, for the 3D scene `vr` or `mip`, `turned`, `cut` and
/// `slice`, `planes` for the plane views in the column or `quad` for them large, `measured`
/// for one of each kind of annotation in the axial plane, `first` for the first series listed in place of the one Voxa opens itself, `stepped` for the next volume of a series that holds several, `labelling` for the strip in the Parts tab as it is on a computer that can label (with `parts`), and `labelled` for a result that Voxa makes itself: the picture starts a run and waits for it, `nolist` for the series list hidden however wide the window is, and `fitted` for the window fitted to the screen as it is when Voxa starts with no size remembered, in place of the size asked for.
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

/// What to do to the 3D scene, once its volume is ready, for a picture that asks for it by
/// the words in `wanted`: a look, a turn, a cut, the slice drawn in it.
pub fn pose(wanted: &str) -> Vec<Action> {
    let asked = |word: &str| asks_for(wanted, word);
    let poses = [
        ("vr", Action::SceneLook(Look::Vr)),
        ("mip", Action::SceneLook(Look::Mip)),
        ("turned", Action::SceneOrbit { by: [70.0, 35.0] }),
        ("cut", Action::CutAdd),
        ("slice", Action::ShowSliceInScene(true)),
        ("planes", Action::SetColumn(ColumnShows::Planes)),
        ("quad", Action::SetMain(MainPanel::Planes)),
        ("parts", Action::SetColumn(ColumnShows::Parts)),
        ("filled", Action::SetOverlay(voxa_render::Overlay::Filled)),
        ("smooth", Action::SetSmooth(true)),
        ("stepped", Action::StepVolume { viewport: 0, by: 1 }),
    ];
    let wanted = poses.into_iter().filter(|(word, _)| asked(word));
    wanted.map(|(_, action)| action).collect()
}

/// What to do to the labelled parts, once labels have been read, for a picture that asks
/// for it by the words in `wanted`: the parts as solids in the scene, the first part faded,
/// the first part selected.
pub fn label_pose(wanted: &str) -> Vec<Action> {
    let asked = |word: &str| asks_for(wanted, word);
    let faded = Action::PartOpacity {
        part: 0,
        opacity: 0.3,
    };
    let poses = [
        ("solids", Action::SceneLook(Look::Parts)),
        ("faded", faded),
        ("chosen", Action::PartSelect(Some(0))),
    ];
    let wanted = poses.into_iter().filter(|(word, _)| asked(word));
    wanted.map(|(_, action)| action).collect()
}

/// Whether a picture is of the 3D scene at rest or while it moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// At rest, drawn in full.
    Still,
    /// Kept moving, drawn as it is while it moves.
    Moving,
}

/// How many frames the window shows what is wanted before its picture is taken: twice as
/// many for a picture of the scene moving, which is first left to come to rest.
pub fn frames_before(scene: Motion) -> u32 {
    if scene == Motion::Moving {
        2 * SETTLE_FRAMES
    } else {
        SETTLE_FRAMES
    }
}

/// For a picture of the scene while it moves, asked for by the word `moving`: after
/// `settled` frames at rest, a turn of half a point one way on one frame and back on the
/// next, which keeps the scene moving and shows the same view. Nothing for the first
/// frames, in which the scene comes to rest and is drawn in full.
pub fn turn_at(scene: Motion, settled: u32) -> Option<Action> {
    let way = if settled.is_multiple_of(2) { 0.5 } else { -0.5 };
    (scene == Motion::Moving && settled >= SETTLE_FRAMES + SETTLE_FRAMES / 2)
        .then_some(Action::SceneOrbit { by: [way, 0.0] })
}

/// For a picture that asks for the first series listed, whatever Voxa would have opened: what
/// shows it. `None` until the index is finished, which is when that series' thumbnail is there.
pub fn first_listed(state: &AppState, has_thumbnail: impl Fn(&SeriesId) -> bool) -> Option<Action> {
    let series = state.index.as_ref()?.series.first()?.info.id.clone();
    has_thumbnail(&series).then_some(Action::SelectSeries {
        viewport: 0,
        series,
    })
}

/// The words a picture asks for a series by, from the variable's `value`: none when it is
/// not set, or set to nothing.
pub fn asked_for(value: Option<String>) -> Option<String> {
    value.filter(|words| !words.trim().is_empty())
}

/// For a picture that asks for a series by `words` from its description: what shows the
/// first listed whose description holds them. `None` until that series' thumbnail is there.
pub fn listed_as(
    state: &AppState,
    words: &str,
    has_thumbnail: impl Fn(&SeriesId) -> bool,
) -> Option<Action> {
    let index = state.index.as_ref()?;
    let listed = index
        .series
        .iter()
        .find(|listed| listed.info.description.contains(words))?;
    let series = listed.info.id.clone();
    has_thumbnail(&series).then_some(Action::SelectSeries {
        viewport: 0,
        series,
    })
}

/// A capture that has been asked for and not yet finished.
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
    /// Whether the picture is to be of the 3D scene at rest or while it moves.
    scene: Motion,
    /// The run of labelling the picture waits for, if it asks for one.
    run: Run,
}

impl Capture {
    /// A capture, if the environment asks for one.
    pub fn from_environment() -> Option<Self> {
        let path = PathBuf::from(std::env::var_os(VARIABLE)?);
        Some(Self {
            path,
            started: Instant::now(),
            settled: 0,
            asked: false,
            first: wants("first"),
            series: asked_for(std::env::var(SERIES_VARIABLE).ok()),
            measured: wants("measured"),
            scene: if wants("moving") {
                Motion::Moving
            } else {
                Motion::Still
            },
            run: Run::asked(&std::env::var(SHOW_VARIABLE).unwrap_or_default()),
        })
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
        // And, if the planes are to be drawn smooth, for the curve they are drawn from.
        let curve = !self.session.state().planes.smooth || self.planes.is_smooth();
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
        let ready = slices && labelled && (no_volume_to_come || self.scene.has_picture() && curve);
        if let Some(capture) = &mut self.capture {
            let pick = capture.pick(self.session.state(), |id| self.thumbnails.contains_key(id));
            capture.step(ctx, ready && pick.is_none());
            if let Some(pick) = pick {
                self.session.dispatch(pick);
            }
        }
    }
}

#[cfg(test)]
mod tests;
