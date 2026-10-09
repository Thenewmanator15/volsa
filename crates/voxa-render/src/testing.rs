//! Volumes to draw and ways to measure the pictures drawn of them, for this crate's tests.

use voxa_core::{Affine, Patient, Point, Volume, Voxel, WindowLevel};

use crate::camera::{Camera, NamedView};
use crate::offscreen_scene::HeadlessScene;
use crate::scene::{Look, SceneParams};
use crate::slice_pipeline::RenderError;

/// The side of every test picture, in pixels.
pub(crate) const SIDE: u32 = 96;
/// The grey above which a pixel counts as drawn on, however faintly, not as the black around.
pub(crate) const FAINT: u8 = 8;
/// Half the grey of the ball's shell: where the edge of the ball is taken to be.
pub(crate) const HALF_SHELL: u8 = 32;
/// Steps of 1 mm along the patient's own axes.
pub(crate) const MM: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// The placement of a grid of `dims` voxels whose steps along columns, rows and slices are
/// `steps`, with the middle of the grid at the patient origin.
pub(crate) fn centred(dims: [u16; 3], steps: [[f64; 3]; 3]) -> Affine<Voxel, Patient> {
    let mut origin = [0.0; 3];
    for (step, count) in steps.iter().zip(dims) {
        let to_middle = (f64::from(count) - 1.0) / 2.0;
        for (at, part) in origin.iter_mut().zip(step) {
            *at -= to_middle * part;
        }
    }
    let [x, y, z] = steps;
    Affine::from_columns(x, y, z, origin)
}

/// The test ball sampled at the centre of every voxel of a grid: 100 within 16 mm of the
/// patient origin, 300 within 6 mm of it, and a bead of 400 within 3 mm of (12, -14, 0),
/// which pokes out at the front on the patient's left. Nothing anywhere else.
pub(crate) fn ball_on(dims: [u16; 3], to_patient: Affine<Voxel, Patient>) -> Volume {
    let [cols, rows, slices] = dims;
    let mut values = Vec::new();
    for slice in 0..slices {
        for row in 0..rows {
            for col in 0..cols {
                let voxel = Point::new(f64::from(col), f64::from(row), f64::from(slice));
                values.push(ball_value(to_patient.apply(voxel)));
            }
        }
    }
    Volume::new(dims.map(u32::from), values, to_patient).expect("adds up")
}

fn ball_value(at: Point<Patient>) -> f32 {
    let from = |x: f64, y: f64, z: f64| {
        ((at.x - x).powi(2) + (at.y - y).powi(2) + (at.z - z).powi(2)).sqrt()
    };
    let from_middle = from(0.0, 0.0, 0.0);
    if from(12.0, -14.0, 0.0) <= 3.0 {
        400.0
    } else if from_middle <= 6.0 {
        300.0
    } else if from_middle <= 16.0 {
        100.0
    } else {
        0.0
    }
}

/// The test ball on 48 voxels of 1 mm each way.
pub(crate) fn ball() -> Volume {
    ball_on([48; 3], centred([48; 3], MM))
}

/// The window the test ball is drawn through: the shell is grey 64, the core 191 and the
/// bead 255.
pub(crate) fn ball_window() -> WindowLevel {
    WindowLevel::new(200.0, 400.0)
}

/// What a test picture of `volume` is drawn from: seen from one side and fitted to the
/// picture, through the ball's window, at full quality, with nothing cut away.
pub(crate) fn scene_of(volume: &Volume, view: NamedView, look: Look) -> SceneParams {
    SceneParams {
        camera: Camera::fitted(view, &volume.corners(), [SIDE as f32; 2]),
        window: ball_window(),
        look,
        cuts: Vec::new(),
        slice: None,
        quality: 1.0,
        skip_empty: false,
    }
}

/// A scene renderer, or `None` with a printed notice on a machine with no graphics adapter.
pub(crate) fn headless_scene() -> Option<HeadlessScene> {
    match HeadlessScene::new() {
        Ok(gpu) => Some(gpu),
        Err(RenderError::NoAdapter) => {
            eprintln!("skipped: no graphics adapter here, not even a software one");
            None
        }
        Err(other) => panic!("{other}"),
    }
}

/// Every pixel of a test picture, as how far across and down it is.
fn pixels() -> impl Iterator<Item = (u32, u32)> {
    (0..SIDE).flat_map(|y| (0..SIDE).map(move |x| (x, y)))
}

/// The grey of the pixel `x` across and `y` down a test picture.
pub(crate) fn grey(rgba: &[u8], x: u32, y: u32) -> u8 {
    rgba[((y * SIDE + x) * 4) as usize]
}

/// The brightest grey in a test picture.
pub(crate) fn brightest(rgba: &[u8]) -> u8 {
    pixels()
        .map(|(x, y)| grey(rgba, x, y))
        .max()
        .expect("a picture has pixels")
}

/// The middle, across and down, of the pixels of a test picture that pass `chosen`, counting
/// from the picture's top left corner to the middle of each pixel.
fn middle_of(rgba: &[u8], chosen: impl Fn(u32, u32, u8) -> bool) -> Option<[f64; 2]> {
    let (mut across, mut down, mut count) = (0.0, 0.0, 0.0);
    for (x, y) in pixels().filter(|&(x, y)| chosen(x, y, grey(rgba, x, y))) {
        across += f64::from(x) + 0.5;
        down += f64::from(y) + 0.5;
        count += 1.0;
    }
    (count > 0.0).then(|| [across / count, down / count])
}

/// The middle of the bead in a brightest-value picture: of the pixels that are nearly white.
pub(crate) fn bead_middle(rgba: &[u8]) -> [f64; 2] {
    middle_of(rgba, |_, _, grey| grey >= 250).expect("the bead is in the picture")
}

/// The first and last column, then the first and last row, that hold a pixel brighter than
/// `above`.
fn drawn_box(rgba: &[u8], above: u8) -> [u32; 4] {
    let drawn = || pixels().filter(|&(x, y)| grey(rgba, x, y) > above);
    let across = || drawn().map(|(x, _)| x);
    let down = || drawn().map(|(_, y)| y);
    let ends = [across().min(), across().max(), down().min(), down().max()];
    ends.map(|end| end.expect("something was drawn"))
}

/// How many pixels wide and how many high the part of a test picture brighter than `above`
/// is.
pub(crate) fn silhouette(rgba: &[u8], above: u8) -> [u32; 2] {
    let [left, right, top, bottom] = drawn_box(rgba, above);
    [right - left + 1, bottom - top + 1]
}

/// How far to the right, in pixels, the top third of what was drawn lies of the bottom
/// third. A ball leans not at all; a ball drawn skewed leans one way.
pub(crate) fn lean(rgba: &[u8]) -> f64 {
    let [_, _, top, bottom] = drawn_box(rgba, FAINT);
    let third = (bottom - top + 1) / 3;
    let middle_across = |from: u32, to: u32| {
        let within = |_: u32, y: u32, grey: u8| (from..to).contains(&y) && grey > FAINT;
        let [across, _] = middle_of(rgba, within).expect("something was drawn there");
        across
    };
    middle_across(top, top + third) - middle_across(bottom + 1 - third, bottom + 1)
}

/// The summed grey of the left half of a test picture, then of its right half.
pub(crate) fn halves(rgba: &[u8]) -> [u64; 2] {
    let sum = |from: u32, to: u32| {
        pixels()
            .filter(|(x, _)| (from..to).contains(x))
            .map(|(x, y)| u64::from(grey(rgba, x, y)))
            .sum()
    };
    [sum(0, SIDE / 2), sum(SIDE / 2, SIDE)]
}

/// The mean difference in grey between two test pictures, pixel for pixel.
pub(crate) fn mean_difference(one: &[u8], other: &[u8]) -> f64 {
    let apart: u64 = pixels()
        .map(|(x, y)| u64::from(grey(one, x, y).abs_diff(grey(other, x, y))))
        .sum();
    apart as f64 / f64::from(SIDE * SIDE)
}
