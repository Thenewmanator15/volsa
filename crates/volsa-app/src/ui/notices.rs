//! The line along the bottom of the window: the disclaimer, and the notice area at its right.
//!
//! The notice area is one button: an icon, the newest notice cut short to fit, and how many
//! there are when there is more than one. A click opens the list of all of them above the
//! line, the newest first, each with a button to dismiss it. There are two kinds of notice,
//! information and something to look at, and nothing is drawn red: Volsa reports no faults in
//! a person, so an error is shown as something to look at.

use egui::{
    Align, Align2, Area, Atom, AtomExt, Button, Context, Frame, Id, Key, Layout, Modifiers, Order,
    Pos2, Rect, Response, RichText, ScrollArea, TextStyle, TextWrapMode, Ui, UiBuilder, Vec2,
    WidgetText,
};

use super::commands::{self, Does, Request};
use super::symbols::{self, Symbol};
use super::theme;
use super::{floating, typing};
use crate::action::Action;
use crate::documents::Mask;
use crate::state::{AppState, Notice, NoticeKind};

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
/// The room left at the sides of the list in a narrow window, in points, and between it and the
/// rail.
const SIDE_ROOM: f32 = 8.0;
/// The list's own name, which it keeps its place under.
const LIST: &str = "volsa: the list of notices";

#[cfg(test)]
mod masking_tests;
#[cfg(test)]
mod tests;

/// How a kind of notice is drawn: its symbol and its colour. Something to look at, and a
/// fault, are both amber.
fn look(ctx: &Context, kind: NoticeKind) -> (Symbol, egui::Color32) {
    let palette = theme::palette(ctx);
    match kind {
        NoticeKind::Info => (Symbol::Info, palette.quiet_text),
        NoticeKind::Warning | NoticeKind::Error => (Symbol::Warning, palette.warning),
    }
}
/// Where the line put the disclaimer and made room for the notice area.
pub struct Line {
    /// The disclaimer as it was laid out.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the tests of the bars find the disclaimer by it")
    )]
    pub disclaimer: Response,
    /// The room made for the notice area; `None` when there is no notice to show.
    area: Option<Rect>,
    /// The top of the line.
    top: f32,
}

/// What the notices say, made fit for the screen: while the patient's details are hidden, no
/// name or identifier of anyone in the export is on the line or in the list, whatever the
/// notice was made from.
struct Said {
    mask: Mask,
}

impl Said {
    /// The notices of `state` as they are shown.
    fn of(state: &AppState) -> Self {
        Self {
            mask: Mask::for_export(state),
        }
    }

    /// A notice as it is shown.
    fn notice(&self, notice: &Notice) -> Notice {
        let hide = |text: &str| self.mask.apply(text).into_owned();
        Notice {
            kind: notice.kind,
            text: hide(&notice.text),
            details: notice.details.iter().map(|detail| hide(detail)).collect(),
            key: notice.key,
        }
    }
}

/// Lay out the line: the disclaimer, and room for the notice area at its right when the line
/// has room for both, or on a line of its own below it when it has not; nothing is laid over
/// the disclaimer, which wraps in a window too narrow for it. The area itself is drawn by
/// [`draw`], after the rest of the window, so that the Tab key reaches it last (R38). Gives
/// where the disclaimer and the room for the area are.
pub fn place(ui: &mut Ui, state: &AppState) -> Line {
    let disclaimer = RichText::new(theme::DISCLAIMER)
        .small()
        .color(theme::palette(ui.ctx()).quiet_text);
    let top = ui.max_rect().top();
    let Some(newest) = state.notices.last() else {
        let disclaimer = ui.add(egui::Label::new(disclaimer).wrap());
        return Line {
            disclaimer,
            area: None,
            top,
        };
    };
    let newest = Said::of(state).notice(newest);
    let height = ui.spacing().interact_size.y;
    let (disclaimer, area) = if beside(ui, &newest, state.notices.len()) {
        ui.horizontal(|ui| {
            let shown = ui.label(disclaimer);
            let width = ui.available_width();
            (shown, ui.allocate_space(Vec2::new(width, height)).1)
        })
        .inner
    } else {
        let shown = ui.add(egui::Label::new(disclaimer).wrap());
        let width = ui.available_width();
        (shown, ui.allocate_space(Vec2::new(width, height)).1)
    };
    Line {
        disclaimer,
        area: Some(area),
        top,
    }
}

/// Draw the notice area in the room [`place`] made for it, and the list of notices above the
/// line while `open`. `open` is changed by a click on the area when the list is open, by Esc
/// and by a click anywhere else. A click on the area while the list is shut asks for it to be
/// opened, which is given back.
pub fn draw(
    ui: &mut Ui,
    state: &AppState,
    line: &Line,
    open: &mut bool,
    actions: &mut Vec<Action>,
) -> Vec<Request> {
    let mut asked = Vec::new();
    let (Some(room), Some(newest)) = (line.area, state.notices.last()) else {
        *open = false;
        return asked;
    };
    let said = Said::of(state);
    let drawn = ui.scope_builder(UiBuilder::new().max_rect(room), |ui| {
        notice_area(ui, state, &said, newest)
    });
    let Some((area, does)) = drawn.inner else {
        *open = false;
        return asked;
    };
    if does.is_some() && *open {
        *open = false;
    } else if let Some(does) = does {
        asked.extend(does.run(state, actions));
    }
    if *open {
        let at = (&said, &area, line.top);
        list(ui.ctx(), &state.notices, at, (open, actions));
    }
    asked
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

/// The notice area, at the right of the room made for it: a button holding the icon, the
/// newest notice cut short to fit, and how many there are. It is the button of the action that
/// shows the notices, and its hover text holds the whole of the newest notice and what more
/// there is to say of it. Gives the button, and what it did if it was clicked.
fn notice_area(
    ui: &mut Ui,
    state: &AppState,
    said: &Said,
    newest: &Notice,
) -> Option<(Response, Option<Does>)> {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let newest = said.notice(newest);
        let (symbol, colour) = look(ui.ctx(), newest.kind);
        let room = Id::new("volsa: the icon of the newest notice");
        let counted = count_of(state.notices.len());
        let written = match &counted {
            Some(said) => format!("{said}, the newest: {}", newest.text),
            None => format!("1 notice: {}", newest.text),
        };
        let more = newest.details.iter().map(String::as_str);
        let note: Vec<&str> = std::iter::once(newest.text.as_str()).chain(more).collect();
        let note = note.join("\n");
        let shows = (written.as_str(), note.as_str());
        let made = commands::button_made_noting(ui, "notices.show", state, shows, |_| {
            let words = RichText::new(&newest.text).small().color(colour);
            let icon = Atom::custom(room, Vec2::splat(ICON));
            let button = Button::new((icon, words.atom_shrink(true)))
                .truncate()
                .small()
                .frame_when_inactive(false);
            match &counted {
                Some(said) => button.right_text(RichText::new(said).small().color(colour)),
                None => button,
            }
        })?;
        if let Some(rect) = made.0.rect(room) {
            symbols::paint(ui.painter(), rect, symbol, colour);
        }
        Some((made.0.response, made.1))
    })
    .inner
}

/// Draw the list of notices above the line, the newest first, each with a button to dismiss
/// it, while `open`; Esc, and a press anywhere but on the list or the area `at.1`, close it,
/// and so does the last notice being dismissed. `at.2` is the top of the line.
fn list(
    ctx: &Context,
    notices: &[Notice],
    at: (&Said, &Response, f32),
    (open, actions): (&mut bool, &mut Vec<Action>),
) {
    let (said, area, top) = at;
    let room = ctx.content_rect();
    // Nothing floats over the scans rail: the list starts no further left than where the
    // pictures do, as the window told the floating windows in the frame before, and is
    // narrower than it would be if that is nearer than its widest.
    let left = floating::area(ctx).left().max(room.left()) + SIDE_ROOM;
    let width = (room.right() - SIDE_ROOM - left).clamp(0.0, LIST_WIDEST);
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
                            row(ui, place, &said.notice(notice), actions);
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
    let (symbol, colour) = look(ui.ctx(), notice.kind);
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
                let said = RichText::new(detail)
                    .small()
                    .color(theme::palette(ui.ctx()).quiet_text);
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
    let escaped = !typing::keys_are_taken(ctx) && ctx.input(|input| input.key_pressed(Key::Escape));
    if escaped {
        ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
    }
    let away = ctx.input(|input| {
        let elsewhere = |at: Pos2| !list.contains(at) && !area.contains(at);
        input.pointer.any_pressed() && input.pointer.interact_pos().is_some_and(elsewhere)
    });
    escaped || away
}
