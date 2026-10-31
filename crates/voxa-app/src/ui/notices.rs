//! The line along the bottom of the window: the disclaimer, and the notice area at its right.
//!
//! The notice area is one button: an icon, the newest notice cut short to fit, and how many
//! there are when there is more than one. A click opens the list of all of them above the
//! line, the newest first, each with a button to dismiss it. There are two kinds of notice,
//! information and something to look at, and nothing is drawn red: Voxa reports no faults in
//! a person, so an error is shown as something to look at.

use egui::{
    Align, Align2, Area, Atom, AtomExt, Button, Context, Frame, Id, Key, Layout, Modifiers, Order,
    Pos2, Rect, Response, RichText, ScrollArea, TextStyle, TextWrapMode, Ui, Vec2, WidgetText,
};

use super::search;
use super::symbols::{self, Symbol};
use super::theme;
use crate::action::Action;
use crate::state::{Notice, NoticeKind};

/// The least of a notice that is shown beside the disclaimer, in points: a notice that would
/// be cut shorter than this goes onto a line of its own.
const LEAST_OF_A_NOTICE: f32 = 120.0;
/// What the button that dismisses a notice says.
const DISMISS: &str = "Dismiss";
/// How wide the icon of a notice is drawn, in points.
const ICON: f32 = 14.0;
/// How wide the list of notices is, at most, in points.
const LIST_WIDEST: f32 = 460.0;
/// How tall the list is, at most, before it scrolls.
const LIST_TALLEST: f32 = 320.0;
/// How far above the line the list ends, in points.
const ABOVE_THE_LINE: f32 = 6.0;
/// The room left at the sides of the list in a narrow window, in points.
const SIDE_ROOM: f32 = 8.0;
/// The list's own name, which it keeps its place under.
const LIST: &str = "voxa: the list of notices";

#[cfg(test)]
mod tests;

/// How a kind of notice is drawn: its symbol and its colour. Something to look at, and a
/// fault, are both amber.
fn look(kind: NoticeKind) -> (Symbol, egui::Color32) {
    match kind {
        NoticeKind::Info => (Symbol::Info, theme::QUIET_TEXT),
        NoticeKind::Warning | NoticeKind::Error => (Symbol::Warning, theme::WARNING),
    }
}

/// Draw the line: the disclaimer, and the notice area. `open` says whether the list of
/// notices is open, and is changed by a click on the notice area, by Esc, and by a click
/// anywhere else. The area goes beside the disclaimer when the line has room for both, and
/// onto a line of its own below it when it has not; nothing is drawn over the disclaimer,
/// which wraps in a window too narrow for it. Gives the disclaimer as it was laid out.
pub fn show(
    ui: &mut Ui,
    notices: &[Notice],
    open: &mut bool,
    actions: &mut Vec<Action>,
) -> Response {
    let disclaimer = RichText::new(theme::DISCLAIMER)
        .small()
        .color(theme::QUIET_TEXT);
    let top = ui.max_rect().top();
    let Some(newest) = notices.last() else {
        *open = false;
        return ui.add(egui::Label::new(disclaimer).wrap());
    };
    let (shown, area) = if beside(ui, newest, notices.len()) {
        ui.horizontal(|ui| {
            let shown = ui.label(disclaimer);
            (shown, notice_area(ui, notices, newest))
        })
        .inner
    } else {
        let shown = ui.add(egui::Label::new(disclaimer).wrap());
        let area = ui.horizontal(|ui| notice_area(ui, notices, newest)).inner;
        (shown, area)
    };
    if area.clicked() {
        *open = !*open;
    }
    if *open {
        list(ui.ctx(), notices, (&area, top), open, actions);
    }
    shown
}

/// The width of `text` in a text style, laid out on one line.
fn wide(ui: &Ui, text: &str, style: TextStyle) -> f32 {
    let text = WidgetText::from(RichText::new(text).text_style(style.clone()));
    let laid = text.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, style);
    laid.size().x
}

/// The words that say how many notices there are, when there is more than one.
fn count_of(count: usize) -> Option<String> {
    (count > 1).then(|| format!("{count} notices"))
}

/// Whether the line has room for the disclaimer, and the notice area with at least
/// [`LEAST_OF_A_NOTICE`] of the newest notice, side by side.
fn beside(ui: &Ui, notice: &Notice, count: usize) -> bool {
    let spacing = ui.spacing();
    let words = wide(ui, &notice.text, TextStyle::Small).min(LEAST_OF_A_NOTICE);
    let counted = count_of(count).map_or(0.0, |said| wide(ui, &said, TextStyle::Small));
    // A button is its atoms with a gap between each, the padding either side, and a point of
    // frame each side.
    let gaps = if counted > 0.0 { 3.0 } else { 1.0 } * spacing.icon_spacing;
    let button = ICON + words + counted + gaps + 2.0 * spacing.button_padding.x + 2.0;
    let needed = wide(ui, theme::DISCLAIMER, TextStyle::Small) + button;
    needed + 2.0 * spacing.item_spacing.x <= ui.available_width()
}

/// The notice area, at the right of what is left of its line: a button holding the icon, the
/// newest notice cut short to fit, and how many there are. Gives the button.
fn notice_area(ui: &mut Ui, notices: &[Notice], newest: &Notice) -> Response {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let (symbol, colour) = look(newest.kind);
        let room = Id::new("voxa: the icon of the newest notice");
        let words = RichText::new(&newest.text).small().color(colour);
        let icon = Atom::custom(room, Vec2::splat(ICON));
        let mut button = Button::new((icon, words.atom_shrink(true)))
            .truncate()
            .small()
            .frame_when_inactive(false);
        let counted = count_of(notices.len());
        if let Some(said) = &counted {
            button = button.right_text(RichText::new(said).small().color(colour));
        }
        let shown = button.atom_ui(ui);
        if let Some(rect) = shown.rect(room) {
            symbols::paint(ui.painter(), rect, symbol, colour);
        }
        let name = match &counted {
            Some(said) => format!("{said}, the newest: {}", newest.text),
            None => format!("1 notice: {}", newest.text),
        };
        ui.ctx()
            .accesskit_node_builder(shown.response.id, |node| node.set_label(name));
        let hover =
            std::iter::once(newest.text.as_str()).chain(newest.details.iter().map(String::as_str));
        shown
            .response
            .on_hover_text(hover.collect::<Vec<_>>().join("\n"))
    })
    .inner
}

/// Draw the list of notices above the line, the newest first, each with a button to dismiss
/// it, while `open`; Esc, and a press anywhere but on the list or the area `at`, close it,
/// and so does the last notice being dismissed. `at.1` is the top of the line.
fn list(
    ctx: &Context,
    notices: &[Notice],
    at: (&Response, f32),
    open: &mut bool,
    actions: &mut Vec<Action>,
) {
    let (area, top) = at;
    let room = ctx.content_rect();
    let width = (room.width() - 2.0 * SIDE_ROOM).clamp(0.0, LIST_WIDEST);
    let tallest = (top - room.top() - 2.0 * SIDE_ROOM).clamp(40.0, LIST_TALLEST);
    let corner = Pos2::new(room.right() - SIDE_ROOM, top - ABOVE_THE_LINE);
    let shown = Area::new(Id::new(LIST))
        .order(Order::Foreground)
        .pivot(Align2::RIGHT_BOTTOM)
        .fixed_pos(corner)
        .show(ctx, |ui| {
            Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(width - 2.0 * ui.spacing().window_margin.leftf());
                ScrollArea::vertical()
                    .max_height(tallest)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        // The newest first, each by its place among them all.
                        for (place, notice) in notices.iter().enumerate().rev() {
                            row(ui, place, notice, actions);
                        }
                    });
            });
        });
    if closed_by_the_person(ctx, shown.response.rect, area.rect) {
        *open = false;
    }
}

/// One notice in the list: its icon, its words in full, what more there is to say, and the
/// button that dismisses it.
fn row(ui: &mut Ui, place: usize, notice: &Notice, actions: &mut Vec<Action>) {
    let (symbol, colour) = look(notice.kind);
    let dismiss = wide(ui, DISMISS, TextStyle::Button) + 2.0 * ui.spacing().button_padding.x + 2.0;
    let gap = ui.spacing().item_spacing.x;
    let words = (ui.available_width() - ICON - dismiss - 2.0 * gap).max(40.0);
    ui.horizontal_top(|ui| {
        let (icon, _) = ui.allocate_exact_size(Vec2::splat(ICON), egui::Sense::hover());
        symbols::paint(ui.painter(), icon, symbol, colour);
        ui.vertical(|ui| {
            ui.set_width(words);
            ui.add(egui::Label::new(RichText::new(&notice.text).color(colour)).wrap());
            for detail in &notice.details {
                let said = RichText::new(detail).small().color(theme::QUIET_TEXT);
                ui.add(egui::Label::new(said).wrap());
            }
        });
        let button = ui.add(Button::new(DISMISS).small());
        ui.ctx().accesskit_node_builder(button.id, |node| {
            node.set_label(format!("{DISMISS}: {}", notice.text));
        });
        if button.clicked() {
            actions.push(Action::DismissNotice(place));
        }
    });
    ui.separator();
}

/// Whether the person has asked to close the list this frame: Esc, unless a text field or the
/// search has use for it, or a press anywhere but on the list at `list` or the area at `area`.
fn closed_by_the_person(ctx: &Context, list: Rect, area: Rect) -> bool {
    let typing = ctx.egui_wants_keyboard_input() || search::holds_keyboard(ctx);
    let escaped = !typing && ctx.input(|input| input.key_pressed(Key::Escape));
    if escaped {
        ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
    }
    let away = ctx.input(|input| {
        let elsewhere = |at: Pos2| !list.contains(at) && !area.contains(at);
        input.pointer.any_pressed() && input.pointer.interact_pos().is_some_and(elsewhere)
    });
    escaped || away
}
