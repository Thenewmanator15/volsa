//! What the rail of scans says of a series and a study, and the small picture of a series: the
//! words on a card and in its hover text, a study's heading, and which card stands for a
//! series of several volumes. The rail itself is drawn in [`super::rail`].

use std::collections::HashMap;

use egui::{Color32, ColorImage};
use voxa_core::{SeriesId, SeriesInfo, StudyInfo, WindowLevel};
use voxa_dicom::Frame;

use crate::naming::is_page;

/// Side of a series' thumbnail, in points.
pub const THUMBNAIL: f32 = 64.0;
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The small picture of each series, once its middle slice has been decoded.
pub type Thumbnails = HashMap<SeriesId, egui::TextureHandle>;

/// Whether a small picture of a series may be made and shown: not of a page, a screenshot or
/// a document while the patient's details are hidden, since such a picture often carries the
/// patient's name in it.
pub fn may_have_thumbnail(info: &SeriesInfo, show_patient: bool) -> bool {
    show_patient || !is_page(info)
}

/// A small grey picture of a frame through a window, at most `size` pixels on its longer
/// side and of the same shape as the frame. A frame with nothing in it gives one black pixel.
pub fn thumbnail(frame: &Frame, wl: WindowLevel, size: u32) -> ColorImage {
    let (rows, cols) = (frame.rows as usize, frame.cols as usize);
    if rows == 0 || cols == 0 || size == 0 || frame.values.len() != rows * cols {
        return ColorImage::filled([1, 1], Color32::BLACK);
    }
    if let Some(colours) = frame.colour.as_ref().filter(|all| all.len() == rows * cols) {
        return in_colour(colours, [rows, cols], size);
    }
    let longest = rows.max(cols);
    let wide = (cols * size as usize / longest).max(1);
    let high = (rows * size as usize / longest).max(1);
    // Each pixel of the picture is the mean of the block of the frame it covers.
    let block = |at: usize, of: usize, from: usize| {
        (at * from / of)..((at + 1) * from / of).max(at * from / of + 1)
    };
    let grey = |x: usize, y: usize| {
        let (mut sum, mut count) = (0.0_f32, 0.0_f32);
        for row in block(y, high, rows) {
            for col in block(x, wide, cols) {
                sum += frame.values.get(row * cols + col).copied().unwrap_or(0.0);
                count += 1.0;
            }
        }
        grey_level((sum / count.max(1.0) - wl.low()) / wl.width)
    };
    let pixels = (0..high).flat_map(|y| (0..wide).map(move |x| grey(x, y)));
    ColorImage::from_gray_iter([wide, high], pixels)
}

/// A small picture of a frame stored in colour, as [`thumbnail`] makes of a grey one: each
/// pixel the mean of the block of the frame it covers.
fn in_colour(colours: &[[u8; 3]], [rows, cols]: [usize; 2], size: u32) -> ColorImage {
    let longest = rows.max(cols);
    let wide = (cols * size as usize / longest).max(1);
    let high = (rows * size as usize / longest).max(1);
    let block = |at: usize, of: usize, from: usize| {
        (at * from / of)..((at + 1) * from / of).max(at * from / of + 1)
    };
    let mean = |x: usize, y: usize| {
        let (mut sum, mut count) = ([0.0_f32; 3], 0.0_f32);
        for row in block(y, high, rows) {
            for col in block(x, wide, cols) {
                let colour = colours.get(row * cols + col).copied().unwrap_or_default();
                for (total, part) in sum.iter_mut().zip(colour) {
                    *total += f32::from(part);
                }
                count += 1.0;
            }
        }
        let [red, green, blue] = sum.map(|total| grey_level(total / count.max(1.0) / 255.0));
        Color32::from_rgb(red, green, blue)
    };
    let pixels = (0..high).flat_map(|y| (0..wide).map(move |x| mean(x, y)));
    ColorImage::new([wide, high], pixels.collect())
}

/// A fraction from black (0) to white (1) as one of 256 greys.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held between 0 and 255 first"
)]
fn grey_level(fraction: f32) -> u8 {
    (fraction.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The series a study's cards stand for, in the study's order: every series of one volume,
/// and one volume of each series that holds several. That one is the volume `shown`, when it
/// is of that series, and otherwise the first.
pub fn one_for_each<'a>(series: &'a [SeriesInfo], shown: Option<&SeriesId>) -> Vec<&'a SeriesInfo> {
    let on_show = shown
        .and_then(|shown| series.iter().find(|info| info.id == *shown))
        .and_then(|info| info.set.as_ref());
    let stands = |info: &&SeriesInfo| match &info.set {
        None => true,
        Some(set) => match on_show {
            Some(shown) if shown.of == set.of => shown.at == set.at,
            _ => set.at == 0,
        },
    };
    series.iter().filter(stands).collect()
}

/// The series whose picture a card carries: the series itself, or the first volume of the
/// series it is a volume of, so that one small picture does for all of them.
pub fn face_of<'a>(series: &'a [SeriesInfo], info: &'a SeriesInfo) -> &'a SeriesInfo {
    let first = info.set.as_ref().and_then(|set| {
        let leads = |other: &&SeriesInfo| {
            let found = other.set.as_ref();
            found.is_some_and(|found| found.of == set.of && found.at == 0)
        };
        series.iter().find(leads)
    });
    first.unwrap_or(info)
}

/// The facts of a series that its card begins its last line with: how many volumes when there
/// are several, how many slices, and how far apart.
pub fn caption(info: &SeriesInfo) -> String {
    // Pictures in colour, such as a page or a screenshot, are no slices of the body.
    if info.colour {
        let one = info.slices == 1;
        return if one {
            "1 picture".to_owned()
        } else {
            format!("{} pictures", info.slices)
        };
    }
    let slices = if info.slices == 1 {
        "1 slice".to_owned()
    } else {
        format!("{} slices", info.slices)
    };
    // To a hundredth of a millimetre, without trailing zeros.
    let apart = (info.slice_thickness_mm() * 100.0).round() / 100.0;
    match &info.set {
        Some(set) => format!("{} volumes · {slices} · {apart} mm", set.count),
        None => format!("{slices} · {apart} mm"),
    }
}

/// What is wrong with a series, one sentence a line; `None` when nothing is.
pub fn problems_text(info: &SeriesInfo) -> Option<String> {
    let lines: Vec<String> = info.problems.iter().map(ToString::to_string).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// The scanner's description of a series, or its number when it has none.
pub(super) fn title(info: &SeriesInfo) -> String {
    if info.description.is_empty() {
        format!("Series {}", info.number)
    } else {
        info.description.clone()
    }
}

/// What resting the pointer on a card shows: everything the card may have had no room for.
/// The whole description, how many slices, what kind of scan it is, and each reason the
/// series is not a volume.
pub fn hover(info: &SeriesInfo) -> String {
    let mut lines = vec![title(info), caption(info)];
    lines.extend(info.scan.lines());
    lines.extend(problems_text(info));
    lines.join("\n")
}

/// The heading of a study: its date in words, then its description.
pub fn study_title(study: &StudyInfo) -> String {
    let date = study.date.as_deref().and_then(date_in_words);
    let date = date.unwrap_or_else(|| "Undated study".to_owned());
    if study.description.is_empty() {
        date
    } else {
        format!("{date} · {}", study.description)
    }
}

/// `20190412` as `12 April 2019`; `None` when the text is not a date written that way.
pub(super) fn date_in_words(yyyymmdd: &str) -> Option<String> {
    if yyyymmdd.len() != 8 || !yyyymmdd.is_ascii() {
        return None;
    }
    let (year, rest) = yyyymmdd.split_at(4);
    let (month, day) = rest.split_at(2);
    let month = MONTHS.get(month.parse::<usize>().ok()?.checked_sub(1)?)?;
    let day = day
        .parse::<u8>()
        .ok()
        .filter(|day| (1..=31).contains(day))?;
    year.parse::<u16>().ok()?;
    Some(format!("{day} {month} {year}"))
}

#[cfg(test)]
mod tests;
