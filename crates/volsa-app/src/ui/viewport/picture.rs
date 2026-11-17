//! Saving the slice a viewport shows as a picture: its own pixels through the window it is
//! shown with, mirrored and turned the way it is shown, one pixel of the file to each pixel of
//! the slice. Nothing about the patient is written: no words over it, no text in the file, only
//! the pixels.

use std::path::{Path, PathBuf};

use volsa_core::{Point, Rescale, WindowLevel};
use volsa_dicom::{Frame, StoredSamples, Values};
use volsa_render::{SliceNumbers, ViewTransform};

use super::super::overlay;
use super::super::scene::export;
use super::Frames;
use crate::state::{AppState, Notice};

/// The name offered for the picture, which says nothing of the scan or the patient.
pub const SLICE_NAME: &str = "volsa-slice";

/// A picture of a slice: four bytes, red, green, blue and opaque, for each pixel, row by row,
/// and its width and height in pixels.
pub type Picture = (Vec<u8>, [u32; 2]);

/// The slice `frame` as it is shown with the window `wl` and the view `view`: mirrored and
/// turned the way the view has it, each pixel grey through the window, or in its own colours
/// for a slice stored in colour. Zoom and pan are left out: the whole slice is saved, at its
/// own size. `None` when the frame holds no pixels or fewer values than its size says.
pub fn drawn(frame: &Frame, wl: WindowLevel, view: &ViewTransform) -> Option<Picture> {
    let (cols, rows) = (frame.cols, frame.rows);
    let count = to_index(cols).saturating_mul(to_index(rows));
    if count == 0 || frame.values.len() < count {
        return None;
    }
    let size = if view.quarter_turns % 2 == 1 {
        [rows, cols]
    } else {
        [cols, rows]
    };
    // At one screen pixel to a pixel of the slice, the view places each of its pixels.
    let upright = ViewTransform {
        zoom: 1.0,
        pan: [0.0, 0.0],
        ..*view
    };
    let to_screen = upright.image_to_screen([cols, rows], size.map(length), 1.0);
    let mut rgba = vec![0; count * 4];
    for (from, value) in frame.values.iter().take(count).enumerate() {
        let (row, col) = (from / to_index(cols), from % to_index(cols));
        let at = to_screen.apply(Point::new(whole(col), whole(row), 0.0));
        let (x, y) = (pixel(at.x, size[0]), pixel(at.y, size[1]));
        let colour = frame.colour.as_ref().and_then(|all| all.get(from).copied());
        let [red, green, blue] = colour.unwrap_or([grey(value, wl); 3]);
        let to = (y * to_index(size[0]) + x) * 4;
        if let Some(place) = rgba.get_mut(to..to + 4) {
            place.copy_from_slice(&[red, green, blue, u8::MAX]);
        }
    }
    Some((rgba, size))
}

/// The slice the active viewport of `state` shows, as it is shown: with its window and its
/// view, mirrored as the convention asks. `None` when no slice is there to be saved.
pub fn shown(state: &AppState, frames: &Frames<'_>) -> Option<Picture> {
    let viewport = state.viewports.get(state.active)?;
    let series = viewport.series.as_ref()?;
    let frame = frames(series, viewport.slice)?;
    let placement = state.series(series)?.info.voxel_to_patient.as_ref();
    let view = overlay::display_view(placement, &viewport.view, state.radiological);
    drawn(&frame, viewport.wl, &view)
}

/// Save the slice the active viewport shows at `path`, as a PNG file of its pixels alone, and
/// say how it went.
pub fn save_to(path: &Path, state: &AppState, frames: &Frames<'_>) -> Notice {
    match shown(state, frames) {
        Some((rgba, size)) => export::save_picture(path, &rgba, size),
        None => export::not_drawn(&NOTHING_SHOWN),
    }
}

/// What is said when there is no slice to save.
const NOTHING_SHOWN: &str = "There is no slice shown to save yet.";

/// Ask where to save the slice the active viewport shows, offering a name that says nothing of
/// the scan, and save it there. `None` when the person chose nowhere; otherwise how it went.
pub fn save(state: &AppState, frames: &Frames<'_>) -> Option<Notice> {
    if shown(state, frames).is_none() {
        return Some(export::not_drawn(&NOTHING_SHOWN));
    }
    let path = choose()?;
    Some(save_to(&path, state, frames))
}

/// Ask the person where to save the picture.
fn choose() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Save this slice as a picture")
        .set_file_name(format!("{SLICE_NAME}.png"))
        .add_filter("PNG picture", &["png"])
        .save_file()
}

/// A value as one of 256 greys through the window: black at its low end and below, white at
/// its high end and above, as the slice is drawn on the screen.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and 255 first"
)]
fn grey(value: f32, wl: WindowLevel) -> u8 {
    let share = (value - wl.low()) / wl.width.max(f32::MIN_POSITIVE);
    let share = if share.is_nan() { 0.0 } else { share };
    (share.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The pixel a place on the picture falls in, from 0 to one less than `of`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held inside the picture first"
)]
fn pixel(at: f64, of: u32) -> usize {
    let last = f64::from(of.saturating_sub(1));
    at.floor().clamp(0.0, last) as usize
}

/// A count of pixels as a length on the screen.
#[expect(
    clippy::cast_precision_loss,
    reason = "a slice is far narrower than an f32 counts exactly"
)]
fn length(count: u32) -> f32 {
    count as f32
}

/// A place in the slice, counted in pixels, as a coordinate.
#[expect(
    clippy::cast_precision_loss,
    reason = "a slice holds far fewer pixels than an f64 counts exactly"
)]
fn whole(at: usize) -> f64 {
    at as f64
}

/// A count of pixels as an index.
fn to_index(count: u32) -> usize {
    usize::try_from(count).unwrap_or(usize::MAX)
}

/// A slice's values as the graphics card is given them: the whole numbers the file stores,
/// 2 bytes each, where they are kept so.
pub(super) fn numbers(values: &Values) -> SliceNumbers<'_> {
    let whole = |numbers, signed, rescale: Rescale| SliceNumbers::Whole {
        numbers,
        signed,
        slope: rescale.slope,
        intercept: rescale.intercept,
    };
    match values.kept() {
        StoredSamples::Unsigned(numbers, rescale) => whole(numbers, false, *rescale),
        StoredSamples::Signed(numbers, rescale) => {
            whole(bytemuck::cast_slice(numbers), true, *rescale)
        }
        StoredSamples::Float(values) => SliceNumbers::Values(values),
    }
}
