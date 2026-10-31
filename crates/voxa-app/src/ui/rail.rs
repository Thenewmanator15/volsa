//! The rail of scans down the left of the window: a card for each scan, under each person's
//! and each study's heading where there are several, and below them the other files, in a
//! group that is closed until it is opened. Folded, the rail is a strip that opens it again.
//!
//! An export can hold hundreds of series, so only the rows in sight are drawn: every row is of
//! a fixed height (see [`rows`]), the rows are kept from one frame to the next (see [`kept`]),
//! and the rows in sight are found among them by their heights.
//!
//! While the patient's details are hidden, nothing here says the patient's name: a person's
//! studies are headed `Person 1`, `Person 2`, and a page's card shows a page in place of its
//! picture, which may carry the name.

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Color32, FontId, Frame, Id, Margin, Rect, RichText, ScrollArea, Sense, Shape, Stroke,
    TextStyle, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use voxa_dicom::Index;

use super::browser::{Thumbnails, face_of, study_title};
use super::commands::{self, Request};
use super::{panel, theme};
use crate::action::Action;
use crate::naming::printable;
use crate::people::Person;
use crate::state::AppState;

mod card;
mod kept;
mod rows;
mod strip;

pub use kept::RailState;
use kept::shown_in;
use rows::Row;

/// How wide the rail is folded: a strip as wide as the tab panel's.
pub const STRIP: f32 = panel::STRIP;
/// What the rail says while no export is open.
const NOTHING_OPEN: &str = "Open a scan export to begin: a folder, a zip file or a DICOMDIR.";
/// The action whose label and hint the heading of the other files reads.
const OTHER_FILES: &str = "rail.other";
/// The room above the heading of the other files, in its row.
const ABOVE_OTHER: f32 = 8.0;

/// What the rail says at its top when an export holds the studies of `count` people.
pub fn people_said(count: usize) -> String {
    format!("This export holds scans of {count} people.")
}

/// Draw the rail down the left of the window, `wide` points across: while `open`, the scans
/// (see [`show`]); folded, its strip. Gives where it is, and what it asked of the window.
pub fn column(
    ui: &mut Ui,
    wide: f32,
    open: bool,
    state: &AppState,
    thumbnails: &Thumbnails,
    rail: &mut RailState,
    actions: &mut Vec<Action>,
) -> (Rect, Vec<Request>) {
    let margin = if open { 8 } else { 4 };
    let frame = Frame::NONE
        .fill(theme::PANEL)
        .inner_margin(Margin::same(margin));
    let side = egui::Panel::left("rail")
        .exact_size(wide)
        .resizable(false)
        .frame(frame);
    let shown = side.show(ui, |ui| {
        if open {
            return show(ui, state, thumbnails, rail, actions);
        }
        let does = strip::show(ui);
        does.and_then(|does| does.run(state, actions))
            .into_iter()
            .collect()
    });
    (shown.response.rect, shown.inner)
}

/// Draw the rail's scans: with an export of several people, how many there are, then the rows
/// in sight. Clicking a card shows its series in the active viewport. Gives what the cards'
/// menus asked of the window.
pub fn show(
    ui: &mut Ui,
    state: &AppState,
    thumbnails: &Thumbnails,
    rail: &mut RailState,
    actions: &mut Vec<Action>,
) -> Vec<Request> {
    let mut requests = Vec::new();
    let Some(index) = &state.index else {
        ui.label(RichText::new(NOTHING_OPEN).color(theme::QUIET_TEXT));
        return requests;
    };
    rail.follow(state);
    let open = rail.other_open;
    let laid = rail.laid(index, state);
    if laid.people.len() > 1 {
        ui.label(RichText::new(people_said(laid.people.len())).color(theme::WARNING));
        ui.add_space(4.0);
    }
    let mut asks = card::Asks {
        state,
        actions,
        requests: &mut requests,
    };
    let from = Sources {
        index,
        people: &laid.people,
        thumbnails,
        open,
    };
    let scroll = ScrollArea::vertical()
        .id_salt("voxa: the rail")
        .auto_shrink(false);
    let toggled = scroll.show_viewport(ui, |ui, sight| {
        ui.set_height(laid.height());
        let (origin, wide) = (ui.max_rect().min, ui.max_rect().width());
        let mut toggled = false;
        let first = laid.first_below(sight.min.y);
        let rows = laid.rows.iter().zip(&laid.tops).enumerate().skip(first);
        for (number, (row, top)) in rows.take_while(|(_, (_, top))| **top < sight.max.y) {
            let size = vec2(wide, row.height());
            let rect = Rect::from_min_size(origin + vec2(0.0, *top), size);
            toggled |= draw(ui, Placed { rect, number }, *row, &from, &mut asks);
        }
        toggled
    });
    if toggled.inner {
        rail.other_open = !rail.other_open;
    }
    requests
}

/// What the rows name by their places, and what they are drawn with.
struct Sources<'a> {
    /// The export's index, whose studies and series the rows name.
    index: &'a Index,
    /// The people the rows name.
    people: &'a [Person],
    /// The small pictures of the series.
    thumbnails: &'a Thumbnails,
    /// Whether the other files are open.
    open: bool,
}

/// Where a row is drawn, and which row of the rail it is.
#[derive(Debug, Clone, Copy)]
struct Placed {
    /// The room it takes.
    rect: Rect,
    /// Its place among the rows, from the top.
    number: usize,
}

/// Draw one row where it is `placed`; gives whether it was the heading of the other files,
/// clicked.
fn draw(
    ui: &mut Ui,
    placed: Placed,
    row: Row,
    from: &Sources<'_>,
    asks: &mut card::Asks<'_>,
) -> bool {
    let rect = placed.rect;
    match row {
        Row::Person(place) => {
            if let Some(person) = from.people.get(place) {
                heading(ui, placed, &person.label, &TextStyle::Body);
            }
        }
        Row::Study(at) => {
            if let Some(study) = from.index.studies.get(at) {
                heading(ui, placed, &study_title(study), &TextStyle::Small);
            }
        }
        Row::Card { study, series } => {
            let all = from
                .index
                .studies
                .get(study)
                .map(|study| study.series.as_slice());
            let Some((all, info)) = all.and_then(|all| Some((all, all.get(series)?))) else {
                return false;
            };
            let card = card::Card {
                info,
                texture: from.thumbnails.get(&face_of(all, info).id),
                selected: shown_in(asks.state) == Some(&info.id),
            };
            let at = Rect::from_min_size(rect.min, vec2(rect.width(), card::HEIGHT));
            card::show(ui, at, &card, asks);
        }
        Row::Other(count) => return other_heading(ui, rect, count, from.open),
    }
    false
}

/// A heading of the rail along the foot of the room it is `placed` in, cut short to fit, and
/// told to a screen reader.
fn heading(ui: &mut Ui, placed: Placed, text: &str, style: &TextStyle) {
    let (rect, text) = (placed.rect, printable(text));
    let height = ui.text_style_height(style);
    let font = style.resolve(ui.style());
    let at = pos2(rect.left() + 2.0, rect.bottom() - height - 4.0);
    let place = Rect::from_min_max(at, rect.max);
    let id = Id::new(("voxa: a heading of the rail", placed.number));
    let response = ui.interact(place, id, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
    if ui.is_rect_visible(rect) {
        let colour = ui.visuals().strong_text_color();
        line(ui, at, text, font, colour, rect.right());
    }
}

/// One line of text from `at`, cut short with an ellipsis before `end`.
fn line(ui: &Ui, at: egui::Pos2, text: String, font: FontId, colour: Color32, end: f32) {
    let mut job = LayoutJob::simple_singleline(text, font, colour);
    job.wrap = TextWrapping::truncate_at_width((end - at.x).max(0.0));
    let painter = ui.painter();
    painter.galley(at, painter.layout_job(job), colour);
}

/// The heading of the other files, which opens and closes them: a triangle that points down
/// while they are open, their name, and how many there are; resting the pointer on it says what
/// they are. Gives whether it was clicked.
fn other_heading(ui: &mut Ui, rect: Rect, count: usize, open: bool) -> bool {
    let Some(command) = commands::find(OTHER_FILES) else {
        return false;
    };
    let place = Rect::from_min_max(rect.min + vec2(0.0, ABOVE_OTHER), rect.max);
    let response = ui.interact(place, Id::new("voxa: the other files"), Sense::click());
    let said = format!("{}, {count}", command.label);
    let kind = WidgetType::CollapsingHeader;
    response.widget_info(|| WidgetInfo::selected(kind, true, open, &said));
    if ui.is_rect_visible(place) {
        let look = ui.style().interact(&response);
        let painter = ui.painter();
        if response.hovered() {
            painter.rect_filled(place, look.corner_radius, look.weak_bg_fill);
        }
        let colour = look.fg_stroke.color;
        let tip = place.left_center() + vec2(10.0, 0.0);
        let points = if open {
            vec![
                tip + vec2(-5.0, -3.0),
                tip + vec2(5.0, -3.0),
                tip + vec2(0.0, 4.0),
            ]
        } else {
            vec![
                tip + vec2(-3.0, -5.0),
                tip + vec2(4.0, 0.0),
                tip + vec2(-3.0, 5.0),
            ]
        };
        painter.add(Shape::convex_polygon(points, colour, Stroke::NONE));
        let font = TextStyle::Body.resolve(ui.style());
        let counted = count.to_string();
        let counted_at = pos2(place.right() - 8.0, place.center().y);
        let align = egui::Align2::RIGHT_CENTER;
        let drawn = painter.text(counted_at, align, &counted, font.clone(), theme::QUIET_TEXT);
        let high = ui.text_style_height(&TextStyle::Body);
        let words_at = pos2(place.left() + 22.0, place.center().y - high / 2.0);
        let strong = ui.visuals().strong_text_color();
        line(
            ui,
            words_at,
            command.label.to_owned(),
            font,
            strong,
            drawn.left() - 6.0,
        );
    }
    response.on_hover_text(command.explains).clicked()
}

#[cfg(test)]
mod card_tests;
#[cfg(test)]
mod open_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod whose_tests;
