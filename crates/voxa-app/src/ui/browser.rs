//! The study browser: studies by date, one card for each series.

use std::collections::HashMap;

use egui::{
    Align2, Color32, ColorImage, CornerRadius, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2,
    vec2,
};
use voxa_core::{SeriesId, SeriesInfo, SeriesKind, SeriesProblem, StudyInfo, WindowLevel};
use voxa_dicom::Frame;

use super::theme;
use crate::action::Action;
use crate::state::AppState;

/// Width of the browser column, in points.
pub const WIDTH: f32 = 240.0;
/// Side of a series' thumbnail, in points.
pub const THUMBNAIL: f32 = 64.0;
const PADDING: f32 = 8.0;
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

/// Where the card of a series was drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    /// The series the card stands for.
    pub series: SeriesId,
    /// The area it takes on the screen.
    pub rect: Rect,
}

/// A small grey picture of a frame through a window, at most `size` pixels on its longer
/// side and of the same shape as the frame. A frame with nothing in it gives one black pixel.
pub fn thumbnail(frame: &Frame, wl: WindowLevel, size: u32) -> ColorImage {
    let (rows, cols) = (frame.rows as usize, frame.cols as usize);
    if rows == 0 || cols == 0 || size == 0 || frame.values.len() != rows * cols {
        return ColorImage::filled([1, 1], Color32::BLACK);
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

/// The second line of a card: how many volumes when there are several, how many slices, and
/// how far apart.
pub fn caption(info: &SeriesInfo) -> String {
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

/// The third line of a card, and whether it is a warning: what the series is when it is not a
/// volume, in a few words. Only something that stops Voxa showing a series in full is a
/// warning; locating images, a single image and colour pictures are as they should be.
pub fn note(info: &SeriesInfo) -> Option<(String, bool)> {
    let warning = |problem: &SeriesProblem| Some((format!("! {}", problem.brief()), true));
    match info.kind() {
        SeriesKind::Volume => None,
        SeriesKind::Stack(problem) => warning(&problem),
        SeriesKind::Unreadable(name) => warning(&SeriesProblem::UnsupportedTransferSyntax(name)),
        kind => kind.says().map(|words| (words.to_owned(), false)),
    }
}

/// What a card is headed: the scanner's description, or the series' number when it has none.
fn title(info: &SeriesInfo) -> String {
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

/// Draw the browser and return where each series' card landed. Clicking a card asks for that
/// series in the active viewport. The patient's name never appears here.
pub fn show(
    ui: &mut Ui,
    state: &AppState,
    thumbnails: &Thumbnails,
    actions: &mut Vec<Action>,
) -> Vec<Card> {
    let mut cards = Vec::new();
    ui.set_max_width(WIDTH - 2.0 * PADDING);
    let Some(index) = &state.index else {
        ui.label(
            RichText::new("Open a scan export to begin: a folder, a zip file or a DICOMDIR.")
                .color(theme::QUIET_TEXT),
        );
        return cards;
    };
    let shown = state
        .viewports
        .get(state.active)
        .and_then(|viewport| viewport.series.as_ref());
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            for study in &index.studies {
                ui.label(RichText::new(study_title(study)).strong());
                for info in one_for_each(&study.series, shown) {
                    let texture = thumbnails.get(&face_of(&study.series, info).id);
                    let response = card(ui, info, texture, shown == Some(&info.id));
                    if response.clicked() {
                        actions.push(Action::SelectSeries {
                            viewport: state.active,
                            series: info.id.clone(),
                        });
                    }
                    cards.push(Card {
                        series: info.id.clone(),
                        rect: response.rect,
                    });
                }
                ui.add_space(PADDING);
            }
        });
    cards
}

/// One series: its thumbnail, its description, how many slices, and a mark if it has problems.
fn card(
    ui: &mut Ui,
    info: &SeriesInfo,
    texture: Option<&egui::TextureHandle>,
    selected: bool,
) -> egui::Response {
    let size = vec2(ui.available_width(), THUMBNAIL + 2.0 * PADDING);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let rounded = CornerRadius::same(theme::ROUNDING);
    let painter = ui.painter_at(rect);
    let fill = if response.hovered() {
        theme::RAISED
    } else {
        theme::PANEL
    };
    painter.rect_filled(rect, rounded, fill);
    if selected {
        painter.rect_stroke(
            rect,
            rounded,
            Stroke::new(1.5, theme::ACCENT),
            StrokeKind::Inside,
        );
    }
    let picture = Rect::from_min_size(
        rect.min + vec2(PADDING, PADDING),
        vec2(THUMBNAIL, THUMBNAIL),
    );
    painter.rect_filled(picture, CornerRadius::same(4), Color32::BLACK);
    if let Some(texture) = texture {
        // Centred in its square, keeping the picture's own shape.
        let fitted = texture.size_vec2() * (THUMBNAIL / texture.size_vec2().max_elem().max(1.0));
        let placed = Rect::from_center_size(picture.center(), fitted);
        let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), placed, whole, Color32::WHITE);
    }
    words(ui, &painter, info, picture.right_top() + vec2(PADDING, 0.0));
    response.on_hover_text(hover(info))
}

/// The words on a card, from `at` downwards: what the series is headed, how many slices, what
/// kind of scan it is, and a note of what it is when it is not a volume (the pointer over the
/// card shows the rest).
fn words(ui: &Ui, painter: &egui::Painter, info: &SeriesInfo, at: egui::Pos2) {
    let body = egui::TextStyle::Body.resolve(ui.style());
    let small = egui::TextStyle::Small.resolve(ui.style());
    let strong = ui.visuals().strong_text_color();
    let line = |down: f32, text: String, font, colour| {
        painter.text(at + vec2(0.0, down), Align2::LEFT_TOP, text, font, colour);
    };
    line(4.0, title(info), body, strong);
    line(26.0, caption(info), small.clone(), theme::QUIET_TEXT);
    let mut down = 42.0;
    if let Some(scan) = info.scan.line() {
        line(down, scan, small.clone(), theme::QUIET_TEXT);
        down += 14.0;
    }
    if let Some((text, warning)) = note(info) {
        let colour = if warning {
            theme::WARNING
        } else {
            theme::QUIET_TEXT
        };
        line(down, text, small, colour);
    }
}

#[cfg(test)]
mod tests;
