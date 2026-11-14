//! What is written over the 3D scene, and what its picture is drawn from.

use std::time::Duration;

use egui::{WidgetInfo, WidgetType};
use volsa_render::{Camera, SceneParams, SlicePlane};

use super::super::overlay::{EdgeLabels, letter};
use super::super::parts::labelling::solids_in_words;
use super::super::pen::LETTER_ROOM;
use super::parts::CAPTION;
use crate::labels::look_drawn;
use crate::naming::plain_name;
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

/// What is said first of solids from labels Volsa made when the model's own check of them failed.
const CHECK_FAILED: &str = "The model's own check of these labels failed.";

/// What the scene says of its solids in the Parts look: that they are labels made elsewhere,
/// drawn as given, or, for labels that Volsa made, with what, where and when, and first of all,
/// as the list says it, that the model's own check of them failed, if it did.
pub fn solids(state: &AppState) -> String {
    let labels = &state.labels;
    let Some(made) = &labels.made else {
        return CAPTION.to_owned();
    };
    let said = solids_in_words(made);
    if labels
        .verdict
        .as_ref()
        .is_some_and(|verdict| !verdict.passed)
    {
        format!("{CHECK_FAILED} {said}")
    } else {
        said
    }
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

/// How finely the scene is drawn: while the camera is moving and for a moment after, as
/// finely as the graphics device keeps up with, going by how long it took over the last
/// picture it drew in full, if it has drawn one; then in full.
pub fn quality(since_moved: Duration, full_took: Option<Duration>) -> f32 {
    if since_moved < SETTLING {
        super::pace::moving_quality(full_took)
    } else {
        1.0
    }
}

/// How long until [`quality`] becomes full, if it is not yet.
pub fn until_full_quality(since_moved: Duration) -> Option<Duration> {
    SETTLING
        .checked_sub(since_moved)
        .filter(|left| !left.is_zero())
}

/// What a screen reader is told of the 3D scene, a picture with no words of its own: that it is
/// the 3D view, and of which scan.
pub fn widget(state: &AppState) -> WidgetInfo {
    let scan = state.scene.series.as_ref().and_then(|id| state.series(id));
    let said = match scan {
        Some(listed) => format!("3D view of {}", plain_name(&listed.info)),
        None => "3D view, no scan".to_owned(),
    };
    WidgetInfo::labeled(WidgetType::Image, true, said)
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

/// What is said over a preview of the volume while the rest of it is decoded.
pub fn preview_text(scene: &Scene) -> Option<String> {
    match scene.status {
        SceneStatus::Building { done, of } if scene.preview => Some(format!(
            "Preview: loading full detail ({done} of {of} slices)"
        )),
        _ => None,
    }
}

/// What the scene's picture is drawn from: its camera, look and cuts, the window of the
/// active viewport, and the slice that viewport shows when it is to be drawn in the scene.
/// `None` unless the scene's volume is ready, or a preview of it is there to draw.
pub fn scene_params(state: &AppState, quality: f32) -> Option<SceneParams> {
    let scene = &state.scene;
    if scene.status != SceneStatus::Ready && preview_text(scene).is_none() {
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
        window: if crate::susceptibility::in_the_scene(state) {
            crate::susceptibility::window_in_3d()
        } else {
            viewport.wl
        },
        look: look_drawn(state),
        cuts: scene.cuts.clone(),
        slice: scene.slice_shown.then(plane_of_the_slice).flatten(),
        quality,
        skip_empty: true,
    })
}

#[cfg(test)]
mod tests;
