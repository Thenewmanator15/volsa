//! Saving a picture of the window to a file, for checking by eye and for the documentation.
//!
//! Set the environment variable `VOXA_CAPTURE` to a `.png` path. Voxa then saves one picture
//! of its own window, once the first viewport has something to show or after a wait, and
//! closes. Only Voxa's window is in the picture, never the rest of the screen. With
//! `VOXA_CAPTURE_SIZE` set as well, to a size such as `900x600`, the window is that size.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use voxa_core::{Patient, Point, SeriesId, Volume};
use voxa_render::Look;
use voxa_volume::{PlaneKind, ellipse_stats};

use super::VoxaApp;
use super::viewport::Gpu;
use crate::action::Action;
use crate::annotations::Shape;
use crate::planes::{ColumnShows, plane};
use crate::scene::MainPanel;
use crate::scene::SceneStatus;
use crate::state::AppState;

/// The environment variable naming the file to save.
pub const VARIABLE: &str = "VOXA_CAPTURE";
/// The environment variable naming what to have in the picture, separated by commas: the
/// windows `tags`, `keys` and `about`, for the 3D scene `vr` or `mip`, `turned`, `cut` and
/// `slice`, `planes` for the plane views in the column or `quad` for them large, `measured`
/// for one of each kind of annotation in the axial plane, and `first` for the first series listed in place of the one Voxa opens itself.
pub const SHOW_VARIABLE: &str = "VOXA_CAPTURE_SHOW";
/// The environment variable giving the size of the window for the picture, in points, as
/// width `x` height: `900x600`.
pub const SIZE_VARIABLE: &str = "VOXA_CAPTURE_SIZE";
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

/// What to do to the 3D scene, once its volume is ready, for a picture that asks for it by
/// the words in `wanted`: a look, a turn, a cut, the slice drawn in it.
pub fn pose(wanted: &str) -> Vec<Action> {
    let asked = |word: &str| wanted.split(',').any(|part| part.trim() == word);
    let poses = [
        ("vr", Action::SceneLook(Look::Vr)),
        ("mip", Action::SceneLook(Look::Mip)),
        ("turned", Action::SceneOrbit { by: [70.0, 35.0] }),
        ("cut", Action::CutAdd),
        ("slice", Action::ShowSliceInScene(true)),
        ("planes", Action::SetColumn(ColumnShows::Planes)),
        ("quad", Action::SetMain(MainPanel::Planes)),
    ];
    let wanted = poses.into_iter().filter(|(word, _)| asked(word));
    wanted.map(|(_, action)| action).collect()
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

/// For a picture that asks for it with the word `measured`: a ruler, an angle, a region and
/// a marker in the axial plane through the position, as the tools would have made them from
/// `scan`, the scene's volume. Nothing while there is no volume ready.
pub fn sample_annotations(state: &AppState, scan: &Volume) -> Vec<Action> {
    let Some((axial, corners)) = plane(state, PlaneKind::Axial).zip(state.scene.corners) else {
        return Vec::new();
    };
    // A fifth of how far the volume reaches from the middle of the plane, as the unit.
    let reach = corners.iter().fold(0.0_f64, |reach, corner| {
        let ([across, down], _) = axial.of(*corner);
        reach.max(across.abs()).max(down.abs())
    });
    let unit = reach / 5.0;
    let at = |across: f64, down: f64| xyz(axial.at([across * unit, down * unit]));
    let radii_mm = [unit, 0.6 * unit];
    let inside = ellipse_stats(scan, &axial, [0.0, -2.6 * unit], radii_mm);
    let shapes = [
        Shape::Ruler {
            a: at(-2.0, -1.0),
            b: at(2.0, -1.0),
        },
        Shape::Angle {
            a: at(2.0, 2.5),
            vertex: at(0.0, 0.8),
            b: at(-2.0, 2.5),
        },
        Shape::Region {
            centre: at(0.0, -2.6),
            across: axial.across,
            down: axial.down,
            radii_mm,
            stats: inside,
        },
        Shape::Marker { at: at(3.0, 0.5) },
    ];
    shapes.into_iter().map(Action::AnnotationAdd).collect()
}

fn xyz(point: Point<Patient>) -> [f64; 3] {
    [point.x, point.y, point.z]
}

/// A capture that has been asked for and not yet finished.
pub struct Capture {
    path: PathBuf,
    started: Instant,
    settled: u32,
    asked: bool,
    /// Whether the first series listed has still to be shown for the picture.
    first: bool,
    /// Whether one of each kind of annotation has still to be made for the picture.
    measured: bool,
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
            first: std::env::var(SHOW_VARIABLE).is_ok_and(|wanted| wanted.contains("first")),
            measured: std::env::var(SHOW_VARIABLE).is_ok_and(|wanted| wanted.contains("measured")),
        })
    }

    /// What shows the series the picture asks for, when it asks for one: given once.
    pub fn pick(&mut self, state: &AppState, has: impl Fn(&SeriesId) -> bool) -> Option<Action> {
        let pick = first_listed(state, has).filter(|_| self.first)?;
        self.first = false;
        Some(pick)
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
        let due = self.settled >= SETTLE_FRAMES || self.started.elapsed() >= PATIENCE;
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
        if self.session.state().scene.status == SceneStatus::Ready {
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
        }
        // A picture of the window waits for the slice and, if there is to be one, the volume.
        let no_volume_to_come = !matches!(
            self.session.state().scene.status,
            SceneStatus::Building { .. } | SceneStatus::Ready
        );
        let slices = self.slices_hidden() || self.gpus.first().is_some_and(Gpu::has_picture);
        let ready = slices && (no_volume_to_come || self.scene.has_picture());
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
mod tests {
    use super::*;

    #[test]
    fn a_picture_can_ask_for_the_scene_to_be_posed() {
        use voxa_render::Look;
        assert_eq!(pose("tags,keys"), []);
        assert_eq!(pose("cut"), [Action::CutAdd]);
        let all = pose("about, mip, turned, cut, slice");
        let expected = [
            Action::SceneLook(Look::Mip),
            Action::SceneOrbit { by: [70.0, 35.0] },
            Action::CutAdd,
            Action::ShowSliceInScene(true),
        ];
        assert_eq!(
            all, expected,
            "in an order that does not depend on how they were asked"
        );
        assert_eq!(pose("vr"), [Action::SceneLook(Look::Vr)]);
    }

    #[test]
    fn a_picture_can_ask_for_the_first_series_listed_once_the_index_is_finished() {
        let mut state = AppState::default();
        assert_eq!(
            first_listed(&state, |_| true),
            None,
            "nothing is listed yet"
        );
        state.index = Some(crate::testing::with_odd_series());
        let listed = state.index.as_ref().and_then(|index| index.series.first());
        let series = listed.map(|listed| listed.info.id.clone()).expect("listed");
        let unfinished = first_listed(&state, |_| false);
        assert_eq!(
            unfinished, None,
            "its thumbnail comes once the index is finished"
        );
        let (viewport, shown) = (0, first_listed(&state, |_| true));
        assert_eq!(shown, Some(Action::SelectSeries { viewport, series }));
    }

    #[test]
    fn a_window_size_is_read_as_width_by_height() {
        assert_eq!(size_from("900x600"), Some([900.0, 600.0]));
        assert_eq!(size_from(" 1440 x 900 "), Some([1440.0, 900.0]));
        assert_eq!(size_from("900"), None);
        assert_eq!(size_from("0x600"), None, "a window has to have a size");
        assert_eq!(size_from("wide x tall"), None);
    }

    #[test]
    fn a_picture_can_ask_for_one_of_each_kind_of_annotation_in_the_axial_plane() {
        use voxa_core::Volume;
        use voxa_volume::PlaneKind;

        use crate::annotations::Shape;
        use crate::planes::plane;
        use crate::testing::{THICK_SCAN, id, thick_scan_ready};
        let state = thick_scan_ready();
        let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
        let placement = info.voxel_to_patient.expect("placed");
        let scan = Volume::new([4, 4, 30], vec![5.0; 480], placement).expect("adds up");
        let made = sample_annotations(&state, &scan);
        let shapes: Vec<&Shape> = made
            .iter()
            .filter_map(|action| match action {
                Action::AnnotationAdd(shape) => Some(shape),
                _ => None,
            })
            .collect();
        let kinds: Vec<&str> = shapes.iter().map(|shape| shape.kind()).collect();
        assert_eq!(kinds, ["Ruler", "Angle", "Region", "Marker"]);
        assert_eq!(made.len(), 4, "and nothing else");
        let axial = plane(&state, PlaneKind::Axial).expect("a plane");
        let on_the_plane = |at: &[f64; 3]| {
            let [x, y, z] = *at;
            axial.of(voxa_core::Point::new(x, y, z)).1.abs() < 1.0e-9
        };
        for shape in &shapes {
            assert!(shape.points().iter().all(on_the_plane), "{shape:?}");
        }
        assert_eq!(
            sample_annotations(&AppState::default(), &scan),
            [],
            "nothing with no volume"
        );
    }

    #[test]
    fn a_picture_of_the_window_neither_shows_nor_changes_what_is_kept_between_runs() {
        let folder = || Some(PathBuf::from("where settings are kept"));
        assert_eq!(kept_between_runs(false, folder()), folder());
        assert_eq!(kept_between_runs(true, folder()), None);
        assert_eq!(kept_between_runs(false, None), None);
    }
}
