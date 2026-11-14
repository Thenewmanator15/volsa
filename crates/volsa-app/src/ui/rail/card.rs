//! The card of one series in the rail: its small picture, its plain name, the scanner's own
//! description beneath it, a line of facts, a mark when something stops the series being shown
//! in full, and a menu that shows it in the selected view or shows its file's tags.
//!
//! The words on a card are cut short with an ellipsis to fit; resting the pointer on it shows
//! them whole. Words from the files are drawn without the characters that would turn, hide or
//! break them.

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Align2, Button, Color32, CornerRadius, FontId, Id, Painter, Popup, Pos2, Rect, Response, Sense,
    Stroke, StrokeKind, TextStyle, TextureHandle, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use volsa_core::{SeriesId, SeriesInfo};

use super::walk;
use crate::action::Action;
use crate::naming::{plain_name, printable};
use crate::state::AppState;
use crate::ui::browser::{THUMBNAIL, caption, hover, may_have_thumbnail, title};
use crate::ui::commands::{self, Request, TagsOf};
use crate::ui::symbols::{self, Symbol};
use crate::ui::theme;
use crate::ui::viewport::Carried;

/// How tall a card is.
pub const HEIGHT: f32 = THUMBNAIL + 2.0 * PADDING;
/// The room around a card's picture and words.
pub(super) const PADDING: f32 = 8.0;
/// The side of the button that opens a card's menu.
pub(super) const DOTS: f32 = 22.0;
/// The side of the mark of a series that cannot be shown in full.
const MARK: f32 = 16.0;
/// What a screen reader calls the mark of a series that cannot be shown in full. It is said after
/// the series' name, so it says what Volsa can do with the series and nothing of the body in it.
pub const NOT_SHOWN_IN_FULL: &str = "cannot be shown in full";
/// The item of a card's menu that shows its series in the active viewport.
pub const SHOW_HERE: &str = "Show in the selected view";

/// What a card shows.
pub struct Card<'a> {
    /// The series it stands for.
    pub info: &'a SeriesInfo,
    /// Its small picture, if one has been made.
    pub texture: Option<&'a TextureHandle>,
    /// Whether the active viewport shows it.
    pub selected: bool,
}

/// What a card's clicks ask for, and of what state.
pub struct Asks<'a> {
    /// The state the card is drawn in.
    pub state: &'a AppState,
    /// Where a change of state goes.
    pub actions: &'a mut Vec<Action>,
    /// Where what is asked of the window goes.
    pub requests: &'a mut Vec<Request>,
}

/// What a screen reader calls the card of a series whose plain name is `name`, without whether
/// it is selected or has a problem: the plain name, then what the card says of its slices. Two
/// cards that share a plain name are told apart by the second part. The plain name is given so
/// that the card works it out once.
fn called(name: &str, info: &SeriesInfo) -> String {
    format!("{name}, {}", caption(info))
}

/// What a screen reader calls the card of a series: see [`called`], with the plain name worked
/// out. The tests ask for it by the series alone.
#[cfg(test)]
pub fn name_of(info: &SeriesInfo) -> String {
    called(&plain_name(info), info)
}

/// What a screen reader says of a card called `name` (see [`called`]): the name, then
/// ", selected" while it is the one shown and ", cannot be shown in full" while something stops
/// the series being shown in full.
pub fn said(name: &str, selected: bool, problem: bool) -> String {
    let mut said = name.to_owned();
    if selected {
        said.push_str(", selected");
    }
    if problem {
        said.push_str(", ");
        said.push_str(NOT_SHOWN_IN_FULL);
    }
    said
}

/// What the button of a card's menu is called by a screen reader, for the card called `name`.
pub fn dots_name(name: &str) -> String {
    format!("More for {name}")
}

/// What resting the pointer on a card shows: its plain name, then all [`hover`] says.
pub fn hover_of(info: &SeriesInfo) -> String {
    let said = hover(info);
    let lines = std::iter::once(plain_name(info)).chain(said.lines().map(printable));
    lines.collect::<Vec<_>>().join("\n")
}

/// The last line of a card: how many slices and how far apart, then the kind of scan.
pub fn facts(info: &SeriesInfo) -> String {
    match info.scan.line() {
        Some(scan) => format!("{} · {scan}", caption(info)),
        None => caption(info),
    }
}

/// Draw a card in `rect`. A click shows its series in the active viewport; a click with the
/// other button, or on its `⋯`, opens its menu. Gives whether the card, or the button of its menu,
/// has the keyboard.
pub fn show(ui: &mut Ui, rect: Rect, card: &Card<'_>, asks: &mut Asks<'_>) -> bool {
    let info = card.info;
    let name = plain_name(info);
    let id = id_of(&info.id);
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let called = called(&name, info);
    let said = said(&called, card.selected, info.kind().is_fault());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &said));
    let picture = Rect::from_min_size(
        rect.min + vec2(PADDING, PADDING),
        vec2(THUMBNAIL, THUMBNAIL),
    );
    let dots = Rect::from_min_size(
        pos2(rect.right() - 4.0 - DOTS, rect.top() + 4.0),
        vec2(DOTS, DOTS),
    );
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let show_patient = asks.state.show_patient;
        let (lit, focused) = (response.hovered(), response.has_focus());
        face(&painter, rect, picture, (card, lit, focused), show_patient);
        let at = picture.right_top() + vec2(PADDING, 0.0);
        let lines = [name.clone(), printable(&title(info)), facts(info)];
        let ends = [dots.left() - 2.0, rect.right() - PADDING];
        words(ui, &painter, lines, at, ends);
    }
    mark(ui, picture, info, id);
    let dots = dots_button(ui, dots, id, &called);
    // Its words are put together only while the pointer rests on the card.
    let response = response.on_hover_ui(|ui| {
        ui.label(hover_of(info));
    });
    // Carried by the pointer, it can be let go over any viewport to be shown there.
    response.dnd_set_drag_payload(Carried {
        series: info.id.clone(),
        name: name.clone(),
    });
    if response.clicked() {
        asks.actions.push(select(asks.state, &info.id));
    }
    response.context_menu(|ui| menu(ui, &info.id, asks));
    Popup::menu(&dots).show(|ui| menu(ui, &info.id, asks));
    walk::holds_keys(ui, &response, &dots)
}

/// The identifier of the card of the series `series`.
pub(super) fn id_of(series: &SeriesId) -> Id {
    Id::new(("volsa: the card of", series))
}

/// Show `series` in the active viewport.
fn select(state: &AppState, series: &SeriesId) -> Action {
    Action::SelectSeries {
        viewport: state.active,
        series: series.clone(),
    }
}

/// The card's ground, its outline when it is the one shown, and its picture: the small
/// picture, or a page drawn in its place for a page while the patient's details are hidden.
fn face(
    painter: &Painter,
    rect: Rect,
    picture: Rect,
    (card, lit, focused): (&Card<'_>, bool, bool),
    show_patient: bool,
) {
    ground(painter, rect, (lit, focused), card.selected);
    painter.rect_filled(picture, CornerRadius::same(4), Color32::BLACK);
    if !may_have_thumbnail(card.info, show_patient) {
        page(painter, picture);
    } else if let Some(texture) = card.texture {
        // Centred in its square, keeping the picture's own shape.
        let fitted = texture.size_vec2() * (THUMBNAIL / texture.size_vec2().max_elem().max(1.0));
        let placed = Rect::from_center_size(picture.center(), fitted);
        let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), placed, whole, Color32::WHITE);
    }
}

/// A card's ground: raised while the pointer is on it, outlined when it is the one shown, and
/// ringed in the theme's own colour of focus while it has the keyboard. `(lit, focused)` says
/// whether the pointer is on it and whether it has the keyboard.
pub(super) fn ground(painter: &Painter, rect: Rect, (lit, focused): (bool, bool), selected: bool) {
    let rounded = CornerRadius::same(theme::ROUNDING);
    let palette = theme::palette(painter.ctx());
    let fill = if lit { palette.raised } else { palette.panel };
    painter.rect_filled(rect, rounded, fill);
    if selected {
        let outline = Stroke::new(1.5, palette.accent);
        painter.rect_stroke(rect, rounded, outline, StrokeKind::Inside);
    }
    if focused {
        let ring = painter.ctx().global_style().visuals.selection.stroke;
        painter.rect_stroke(rect.shrink(1.0), rounded, ring, StrokeKind::Inside);
    }
}

/// A page with its corner turned down and three lines of writing, in the middle of `square`.
pub(super) fn page(painter: &Painter, square: Rect) {
    let stroke = Stroke::new(1.5, theme::QUIET_TEXT);
    let sheet = Rect::from_center_size(square.center(), vec2(26.0, 34.0));
    let fold = 8.0;
    let outline = vec![
        sheet.left_top(),
        sheet.right_top() - vec2(fold, 0.0),
        sheet.right_top() + vec2(0.0, fold),
        sheet.right_bottom(),
        sheet.left_bottom(),
    ];
    painter.add(egui::Shape::closed_line(outline, stroke));
    let corner = sheet.right_top();
    let turned = [
        corner - vec2(fold, 0.0),
        corner + vec2(-fold, fold),
        corner + vec2(0.0, fold),
    ];
    painter.add(egui::Shape::line(turned.to_vec(), stroke));
    for down in [14.0, 20.0, 26.0] {
        let left = sheet.left_top() + vec2(5.0, down);
        painter.line_segment([left, left + vec2(16.0, 0.0)], stroke);
    }
}

/// The words of a card, from `at` along: the name, ending at `ends[0]` to leave room for the
/// button of the menu, then the description beneath and the facts, ending at `ends[1]`.
pub(super) fn words(
    ui: &Ui,
    painter: &Painter,
    [name, described, facts]: [String; 3],
    at: Pos2,
    ends: [f32; 2],
) {
    let body = TextStyle::Body.resolve(ui.style());
    let small = TextStyle::Small.resolve(ui.style());
    let line = |down: f32, text: String, font: FontId, colour: Color32, end: f32| {
        let mut job = LayoutJob::simple_singleline(text, font, colour);
        job.wrap = TextWrapping::truncate_at_width((end - at.x).max(0.0));
        painter.galley(at + vec2(0.0, down), painter.layout_job(job), colour);
    };
    let strong = ui.visuals().strong_text_color();
    line(4.0, name, body, strong, ends[0]);
    line(
        27.0,
        described,
        small.clone(),
        ui.visuals().text_color(),
        ends[1],
    );
    line(
        45.0,
        facts,
        small,
        theme::palette(ui.ctx()).quiet_text,
        ends[1],
    );
}

/// The mark on the picture of a series that cannot be shown in full: an amber `!`. A screen
/// reader hears [`NOT_SHOWN_IN_FULL`] at the end of the card's name and not from the mark. The
/// card's hover text says what the problem is.
fn mark(ui: &mut Ui, picture: Rect, info: &SeriesInfo, card: Id) {
    if !info.kind().is_fault() {
        return;
    }
    let corner = picture.right_top() + vec2(-MARK - 2.0, 2.0);
    let rect = Rect::from_min_size(corner, vec2(MARK, MARK));
    let response = ui.interact(rect, card.with("mark"), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Image, true, NOT_SHOWN_IN_FULL));
    // The card's own name ends with the same words: said twice, they would only repeat.
    ui.ctx()
        .accesskit_node_builder(response.id, egui::accesskit::Node::set_hidden);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        // Over the picture, which is dark whatever the theme: a chip of the picture's own ground.
        painter.rect_filled(rect, CornerRadius::same(3), theme::BACKGROUND);
        let font = FontId::proportional(13.0);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "!",
            font,
            theme::WARNING,
        );
    }
}

/// The button at the card's top right that opens its menu, named for a screen reader by the
/// card's plain name.
pub(super) fn dots_button(ui: &mut Ui, rect: Rect, card: Id, name: &str) -> Response {
    let response = ui.interact(rect, card.with("menu"), Sense::click());
    let said = dots_name(name);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &said));
    if ui.is_rect_visible(rect) {
        let look = ui.style().interact(&response);
        let painter = ui.painter();
        if response.hovered() {
            painter.rect_filled(rect, look.corner_radius, look.weak_bg_fill);
        }
        let symbol = Rect::from_center_size(rect.center(), vec2(symbols::SIZE, symbols::SIZE));
        symbols::paint(painter, symbol, Symbol::More, look.fg_stroke.color);
    }
    response.on_hover_text(said)
}

/// A card's menu: show its series in the selected view, or show the tags of its first file.
fn menu(ui: &mut Ui, series: &SeriesId, asks: &mut Asks<'_>) {
    if ui.button(SHOW_HERE).clicked() {
        asks.actions.push(select(asks.state, series));
        ui.close();
    }
    let tags = commands::button_made(ui, "tags.show", asks.state, "", |command| {
        Button::new(command.name)
    });
    if let Some((_, Some(_))) = tags {
        asks.requests
            .push(Request::Tags(TagsOf::Series(series.clone())));
        ui.close();
    }
}
