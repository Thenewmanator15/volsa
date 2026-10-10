//! What is written over the 3D scene, and what its picture is drawn from.

use std::time::Duration;

use voxa_render::{Camera, SceneParams, SlicePlane};

use super::super::overlay::{EdgeLabels, letter};
use super::super::pen::LETTER_ROOM;
use crate::labels::look_drawn;
use crate::scene::{Scene, SceneStatus};
use crate::state::AppState;

/// How long after the camera last moved the picture is drawn coarsely, to keep up with it.
const SETTLING: Duration = Duration::from_millis(150);
/// How many times the finer distance within a slice the distance between slices must exceed
/// before the picture says that what lies between is interpolated.
const THICK: f64 = 2.0;

/// What the picture cannot show, in plain words: slices far enough apart that what lies
/// between them is interpolated, and a volume reduced to fit the graphics device. `spacing_mm`
/// is the distance between columns, rows and slices; `halved` how many times the volume was
/// halved each way.
pub fn captions(spacing_mm: [f64; 3], halved: u32) -> Vec<String> {
    let [across, down, between] = spacing_mm;
    let within = across.min(down);
    let mut lines = Vec::new();
    if within > 0.0 && between > THICK * within {
        let apart = format!("{between:.1}");
        let apart = apart.strip_suffix(".0").unwrap_or(&apart);
        lines.push(format!(
            "Slices are {apart} mm apart, so detail between them is interpolated, not measured."
        ));
    }
    let share = match halved {
        0 => None,
        1 => Some("half".to_owned()),
        2 => Some("a quarter".to_owned()),
        3 => Some("an eighth".to_owned()),
        more => Some(format!("1/{}", 1_u64.checked_shl(more).unwrap_or(u64::MAX))),
    };
    lines.extend(
        share.map(|share| format!("Shown at {share} resolution to fit this graphics device.")),
    );
    lines
}

/// A picture narrower than this, inside its margins, has its words beneath the letter at its
/// top: beside the letter, in half the width, they would be a column of single words.
const NARROW: f32 = 480.0;
/// How far beneath the top of such a picture its words begin, to clear the letter.
const UNDER_LETTER: f32 = 20.0;

/// Where the scene's words go in a picture `wide` points across inside its margins: how far
/// beneath its top left corner they begin, and how wide they may run. In a wide picture they
/// stand beside the letter at the top, in half the width; in a narrow one beneath that
/// letter, across the whole width.
pub fn caption_room(wide: f32) -> (f32, f32) {
    if wide < NARROW {
        (UNDER_LETTER, wide)
    } else {
        (0.0, wide / 2.0 - LETTER_ROOM)
    }
}

/// The letter at each edge of the scene, from the way the camera faces: the same letters, by
/// the same rule, as the 2D viewport's.
pub fn edge_letters(camera: &Camera) -> EdgeLabels {
    let [right, down, _] = camera.axes();
    let opposite = |direction: [f64; 3]| direction.map(|part| -part);
    EdgeLabels {
        left: letter(opposite(right)).to_owned(),
        right: letter(right).to_owned(),
        top: letter(opposite(down)).to_owned(),
        bottom: letter(down).to_owned(),
    }
}

/// How finely the scene is drawn: coarsely while the camera is moving and for a moment
/// after, then in full.
pub fn quality(since_moved: Duration) -> f32 {
    if since_moved < SETTLING { 0.5 } else { 1.0 }
}

/// How long until [`quality`] becomes full, if it is not yet.
pub fn until_full_quality(since_moved: Duration) -> Option<Duration> {
    SETTLING
        .checked_sub(since_moved)
        .filter(|left| !left.is_zero())
}

/// What to write in the middle of the scene when there is no picture of a volume to show:
/// how far it has got, or why there is none. `None` when the volume is ready.
pub fn status_text(scene: &Scene) -> Option<String> {
    match &scene.status {
        SceneStatus::Empty => Some("No series is shown here".to_owned()),
        SceneStatus::Building { done, of } => {
            Some(format!("Building the 3D view: {done} of {of} slices"))
        }
        SceneStatus::Unavailable(why) => Some(why.join("\n")),
        SceneStatus::Ready => None,
    }
}

/// What the scene's picture is drawn from: its camera, look and cuts, the window of the
/// active viewport, and the slice that viewport shows when it is to be drawn in the scene.
/// `None` unless the scene's volume is ready.
pub fn scene_params(state: &AppState, quality: f32) -> Option<SceneParams> {
    let scene = &state.scene;
    if scene.status != SceneStatus::Ready {
        return None;
    }
    let viewport = state.viewports.get(state.active)?;
    let plane_of_the_slice = || {
        let info = &state.series(viewport.series.as_ref()?)?.info;
        let placement = info.voxel_to_patient.as_ref()?;
        SlicePlane::of_slice(placement, [info.cols, info.rows], viewport.slice)
    };
    Some(SceneParams {
        camera: scene.camera,
        window: viewport.wl,
        look: look_drawn(state),
        cuts: scene.cuts.clone(),
        slice: scene.slice_shown.then(plane_of_the_slice).flatten(),
        quality,
        skip_empty: true,
    })
}

#[cfg(test)]
mod tests {
    use voxa_core::{Point, WindowLevel};
    use voxa_render::{CutPlane, Look, NamedView};

    use super::*;
    use crate::action::Action;
    use crate::reducer::reduce;
    use crate::testing::{THICK_SCAN, id, three_series};

    fn letters(view: NamedView) -> [String; 4] {
        let camera = Camera::looking(view, Point::new(0.0, 0.0, 0.0), 1.0);
        let EdgeLabels {
            left,
            right,
            top,
            bottom,
        } = edge_letters(&camera);
        [left, right, top, bottom]
    }

    /// A state showing the 30-slice series, whose slices are 5 mm apart going up the
    /// patient, with its volume built.
    fn ready() -> AppState {
        let mut state = AppState::default();
        reduce(&mut state, Action::IndexProgress(three_series()));
        let (viewport, series) = (0, id(THICK_SCAN));
        reduce(
            &mut state,
            Action::SelectSeries {
                viewport,
                series: series.clone(),
            },
        );
        let corners = [Point::new(0.0, 0.0, 0.0); 8];
        reduce(&mut state, Action::VolumeReady { series, corners });
        state
    }

    #[test]
    fn thick_slices_are_said_to_be_interpolated_between() {
        let said = "Slices are 7.5 mm apart, so detail between them is interpolated, not measured.";
        assert_eq!(captions([0.7, 0.7, 7.5], 0), [said]);
        assert_eq!(captions([1.0, 1.0, 1.0], 0), Vec::<String>::new());
        assert_eq!(
            captions([0.5, 0.5, 1.0], 0),
            Vec::<String>::new(),
            "twice is not thick"
        );
        let whole = "Slices are 3 mm apart, so detail between them is interpolated, not measured.";
        assert_eq!(captions([1.0, 0.9, 3.0], 0), [whole]);
    }

    #[test]
    fn a_reduced_volume_says_by_how_much() {
        let said =
            |share: &str| format!("Shown at {share} resolution to fit this graphics device.");
        assert_eq!(captions([1.0, 1.0, 1.0], 1), [said("half")]);
        assert_eq!(captions([1.0, 1.0, 1.0], 2), [said("a quarter")]);
        assert_eq!(captions([1.0, 1.0, 1.0], 3), [said("an eighth")]);
        assert_eq!(captions([1.0, 1.0, 1.0], 5), [said("1/32")]);
        assert_eq!(
            captions([0.7, 0.7, 7.5], 1).len(),
            2,
            "both, when both are so"
        );
    }

    #[test]
    fn the_letters_at_the_edges_follow_the_camera() {
        assert_eq!(letters(NamedView::Anterior), ["R", "L", "S", "I"]);
        assert_eq!(letters(NamedView::Left)[..2], ["A", "P"]);
        assert_eq!(letters(NamedView::Posterior)[..2], ["L", "R"]);
        assert_eq!(letters(NamedView::Inferior), ["R", "L", "A", "P"]);
        assert_eq!(letters(NamedView::Superior), ["L", "R", "A", "P"]);
    }

    #[test]
    fn the_picture_is_coarse_until_the_camera_has_been_still_a_moment() {
        assert_eq!(quality(Duration::ZERO), 0.5);
        assert_eq!(quality(Duration::from_millis(149)), 0.5);
        assert_eq!(quality(Duration::from_millis(150)), 1.0);
        assert_eq!(
            until_full_quality(Duration::from_millis(100)),
            Some(Duration::from_millis(50))
        );
        assert_eq!(until_full_quality(Duration::from_millis(150)), None);
        assert_eq!(until_full_quality(Duration::from_secs(9)), None);
    }

    #[test]
    fn a_scene_with_no_picture_says_where_it_has_got_to() {
        let mut state = AppState::default();
        assert_eq!(
            status_text(&state.scene).as_deref(),
            Some("No series is shown here")
        );
        state.scene.status = SceneStatus::Building { done: 16, of: 30 };
        let building = "Building the 3D view: 16 of 30 slices";
        assert_eq!(status_text(&state.scene).as_deref(), Some(building));
        let why = vec![
            "This series cannot be shown in 3D.".to_owned(),
            "Because.".to_owned(),
        ];
        state.scene.status = SceneStatus::Unavailable(why);
        let both = "This series cannot be shown in 3D.\nBecause.";
        assert_eq!(status_text(&state.scene).as_deref(), Some(both));
        assert_eq!(status_text(&ready().scene), None);
    }

    #[test]
    fn the_picture_is_drawn_from_the_scene_and_the_active_viewports_window() {
        let mut state = ready();
        let wl = WindowLevel::new(321.0, 123.0);
        reduce(&mut state, Action::SetWindow { viewport: 0, wl });
        reduce(&mut state, Action::SceneLook(Look::Mip));
        reduce(&mut state, Action::CutAdd);
        let params = scene_params(&state, 0.5).expect("the volume is ready");
        assert_eq!(
            (params.window, params.look, params.quality),
            (wl, Look::Mip, 0.5)
        );
        assert_eq!(params.camera, state.scene.camera);
        assert_eq!(params.cuts, [CutPlane::facing(&state.scene.camera)]);
        assert!(params.skip_empty && params.slice.is_none());
    }

    #[test]
    fn the_slice_the_viewport_shows_is_drawn_in_the_scene_when_asked() {
        let mut state = ready();
        reduce(
            &mut state,
            Action::SetSlice {
                viewport: 0,
                slice: 3,
            },
        );
        reduce(&mut state, Action::ShowSliceInScene(true));
        let plane = scene_params(&state, 1.0)
            .and_then(|params| params.slice)
            .expect("a plane");
        assert_eq!(plane.point.z, 15.0, "the fourth slice, 5 mm apart");
        assert_eq!(plane.normal.map(f64::abs), [0.0, 0.0, 1.0]);
        reduce(&mut state, Action::ShowSliceInScene(false));
        assert_eq!(
            scene_params(&state, 1.0).and_then(|params| params.slice),
            None
        );
    }

    #[test]
    fn nothing_is_drawn_until_the_volume_is_ready() {
        let mut state = ready();
        state.scene.status = SceneStatus::Building { done: 3, of: 30 };
        assert_eq!(scene_params(&state, 1.0), None);
        assert_eq!(scene_params(&AppState::default(), 1.0), None);
    }
}
