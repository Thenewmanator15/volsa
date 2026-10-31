//! Search: a box that finds actions, parts, scans and help topics, opened with Ctrl+K or `/`.
//!
//! It lists what matches under four headings, the best match first. Arrows move the choice,
//! Enter runs it, a click on a result runs that one, and Esc, or a click anywhere else, closes
//! the box. Its field has the keyboard for as long as the box is open, so that what is typed
//! is never taken for the key of an action.
//!
//! Nothing typed is kept: the box is not written out anywhere, and opening it again finds the
//! field empty.

use egui::{
    Align2, Area, Context, Event, EventFilter, Frame, Id, Key, Modifiers, Order, RichText,
    ScrollArea, TextEdit, Ui, style::ScrollAnimation, vec2,
};

use super::VoxaApp;
use super::theme;
use crate::action::Action;
use crate::state::AppState;
use entries::{Entry, Target, entries_for, stamp};
use matching::ranked;

mod entries;
mod matching;
mod rows;
mod run;

#[cfg(test)]
mod box_tests;
#[cfg(test)]
mod keys_tests;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod view_tests;

/// The box's own name, which it keeps its place under.
const BOX: &str = "voxa: search";
/// The field's own name.
const FIELD: &str = "voxa: search field";
/// Where the window is told, for as long as the box is open, that it has the keyboard.
const HOLDS_KEYBOARD: &str = "voxa: search holds the keyboard";
/// How far below the top of the window the box hangs, in points.
const FROM_TOP: f32 = 48.0;
/// How wide the box is, at most.
const WIDEST: f32 = 480.0;
/// How wide the box is, at least, in a window that leaves it less room.
const NARROWEST: f32 = 240.0;
/// The room left at each side of the box in a narrow window.
const SIDE_ROOM: f32 = 16.0;
/// How tall the results are, at most, before they scroll.
const TALLEST_LIST: f32 = 420.0;
/// What the field says before anything is typed.
const HINT: &str = "Search actions, parts, scans and help";
/// What the list says before anything is typed.
const BEFORE_TYPING: &str = "Type a few letters of what you want to do or find.";
/// What the list says when nothing matches.
const NO_MATCH: &str = "Nothing matches that.";

/// The search box and its list.
#[derive(Default)]
pub struct Search {
    /// Whether the box is open.
    open: bool,
    /// Whether it was opened in the pass now being drawn, whose key presses and clicks are
    /// the ones that opened it.
    fresh: bool,
    /// What is typed.
    query: String,
    /// Which result is chosen, by its place among those listed.
    chosen: usize,
    /// Everything that can be found, as of `built_for`.
    entries: Vec<Entry>,
    /// What [`stamp`] gave when `entries` was made.
    built_for: u64,
    /// What `query` finds among `entries`, as places in it.
    results: Vec<usize>,
    /// The query `results` is for; `None` when `entries` has been made again since.
    results_for: Option<String>,
    /// Whether the results were found again since they were last drawn. The list keeps how far
    /// it was scrolled, whatever it holds, so it is drawn from its first result.
    to_top: bool,
}

/// The keys the box answers to, taken from the pass before the field can take them for its own.
#[derive(Debug, Clone, Copy, Default)]
struct Pressed {
    /// Esc was pressed.
    escape: bool,
    /// Enter was pressed.
    enter: bool,
    /// How many places down the arrow keys moved the choice, up being less.
    down: isize,
}

impl Pressed {
    /// Take the box's keys from the pass.
    fn take(ctx: &Context) -> Self {
        ctx.input_mut(|input| {
            let mut times = |wanted: Key| {
                let pressed = |event: &Event| match event {
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => *key == wanted && modifiers.matches_logically(Modifiers::NONE),
                    _ => false,
                };
                let times = input.events.iter().filter(|event| pressed(event)).count();
                input.consume_key(Modifiers::NONE, wanted);
                isize::try_from(times).unwrap_or(isize::MAX)
            };
            Self {
                escape: times(Key::Escape) > 0,
                enter: times(Key::Enter) > 0,
                down: times(Key::ArrowDown) - times(Key::ArrowUp),
            }
        })
    }
}

impl Search {
    /// Open the box with nothing typed.
    pub fn open(&mut self) {
        self.open = true;
        self.fresh = true;
        self.query.clear();
        self.chosen = 0;
        self.results_for = None;
    }

    /// Close the box and let the keyboard go.
    fn close(&mut self, ctx: &Context) {
        self.open = false;
        self.query.clear();
        holds_keyboard_set(ctx, false);
        ctx.memory_mut(|memory| memory.surrender_focus(Id::new(FIELD)));
    }

    /// Draw the box while it is open; gives the result that was chosen in it, if one was, and
    /// closes the box when one is.
    pub fn show(&mut self, ctx: &Context, state: &AppState) -> Option<Target> {
        if !self.open {
            return None;
        }
        holds_keyboard_set(ctx, true);
        self.list_again_if_changed(state);
        if self.fresh {
            // The text a `/` makes arrives in the pass that opened the box: it is not typed.
            ctx.input_mut(|input| {
                input
                    .events
                    .retain(|event| !matches!(event, Event::Text(_)));
            });
        }
        let pressed = Pressed::take(ctx);
        if pressed.escape {
            self.close(ctx);
            return None;
        }
        let (mut chosen, area) = self.draw(ctx, state, pressed);
        let outside = ctx.input(|input| {
            let away = |at| !area.contains(at);
            input.pointer.any_pressed() && input.pointer.interact_pos().is_some_and(away)
        });
        if pressed.enter && chosen.is_none() {
            chosen = self.target_chosen();
        }
        if chosen.is_some() || (outside && !self.fresh) {
            self.close(ctx);
        }
        self.fresh = false;
        chosen
    }

    /// Make the list of what can be found again, if anything it holds may have changed.
    fn list_again_if_changed(&mut self, state: &AppState) {
        let stamped = stamp(state);
        if self.entries.is_empty() || self.built_for != stamped {
            self.entries = entries_for(state);
            self.built_for = stamped;
            self.results_for = None;
        }
    }

    /// Find again what the query finds, if it is not what was last found for.
    fn find_again_if_changed(&mut self) {
        if self.results_for.as_deref() != Some(self.query.as_str()) {
            self.results = ranked(&self.query, &self.entries);
            self.results_for = Some(self.query.clone());
            // A list found again is another list: its first result is chosen, and in view.
            self.chosen = 0;
            self.to_top = true;
        }
        self.chosen = self.chosen.min(self.results.len().saturating_sub(1));
    }

    /// What is chosen, if anything is.
    fn target_chosen(&self) -> Option<Target> {
        let place = self.results.get(self.chosen)?;
        Some(self.entries.get(*place)?.target.clone())
    }

    /// Draw the box with its field and its results; gives what a click chose, and where the
    /// box is.
    fn draw(
        &mut self,
        ctx: &Context,
        state: &AppState,
        pressed: Pressed,
    ) -> (Option<Target>, egui::Rect) {
        let room = ctx.content_rect();
        let width = (room.width() - 2.0 * SIDE_ROOM).clamp(NARROWEST, WIDEST);
        let tallest = (room.height() - FROM_TOP - 96.0).clamp(80.0, TALLEST_LIST);
        let mut chosen = None;
        let shown = Area::new(Id::new(BOX))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_TOP, vec2(0.0, FROM_TOP))
            .show(ctx, |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(width);
                    self.field(ui);
                    self.find_again_if_changed();
                    let moved = self.move_choice(pressed.down);
                    chosen = self.list(ui, state, tallest, moved);
                    self.to_top = false;
                });
            });
        (chosen, shown.response.rect)
    }

    /// Draw the field, which has the keyboard for as long as the box is open.
    fn field(&mut self, ui: &mut Ui) {
        let id = Id::new(FIELD);
        ui.memory_mut(|memory| memory.request_focus(id));
        // Esc, Tab and the arrows are the box's to answer, not for moving the focus away.
        let keep = EventFilter {
            tab: true,
            horizontal_arrows: true,
            vertical_arrows: true,
            escape: true,
        };
        let field = TextEdit::singleline(&mut self.query)
            .id(id)
            .hint_text(HINT)
            .desired_width(f32::INFINITY)
            .event_filter(keep);
        let response = ui.add(field);
        ui.ctx()
            .accesskit_node_builder(response.id, |node| node.set_label("Search"));
    }

    /// Move the choice down by `places`, round the ends; whether it moved.
    fn move_choice(&mut self, places: isize) -> bool {
        let count = isize::try_from(self.results.len()).unwrap_or(0);
        let chosen = isize::try_from(self.chosen).unwrap_or(0);
        if count == 0 || places == 0 {
            return false;
        }
        self.chosen = usize::try_from((chosen + places).rem_euclid(count)).unwrap_or(0);
        true
    }

    /// Draw the results under their headings; gives the one that was clicked. `follow` brings
    /// the chosen result into view.
    fn list(&self, ui: &mut Ui, state: &AppState, tallest: f32, follow: bool) -> Option<Target> {
        if self.query.trim().is_empty() {
            ui.label(RichText::new(BEFORE_TYPING).color(theme::QUIET_TEXT));
            return None;
        }
        if self.results.is_empty() {
            ui.label(RichText::new(NO_MATCH).color(theme::QUIET_TEXT));
            return None;
        }
        let mut clicked = None;
        let mut under = None;
        let mut area = ScrollArea::vertical()
            .max_height(tallest)
            .auto_shrink([false, true]);
        if self.to_top {
            area = area.vertical_scroll_offset(0.0);
        }
        area.show(ui, |ui| {
            for (position, place) in self.results.iter().enumerate() {
                let Some(entry) = self.entries.get(*place) else {
                    continue;
                };
                if under != Some(entry.heading) {
                    under = Some(entry.heading);
                    rows::heading(ui, entry.heading);
                }
                let chosen = position == self.chosen;
                let response = rows::row(ui, entry, state, chosen);
                if chosen && follow {
                    response.scroll_to_me_animation(None, ScrollAnimation::none());
                }
                if response.clicked() {
                    clicked = Some(entry.target.clone());
                }
            }
        });
        clicked
    }
}

/// Tell the window whether the box has the keyboard.
fn holds_keyboard_set(ctx: &Context, holds: bool) {
    ctx.data_mut(|data| data.insert_temp(Id::new(HOLDS_KEYBOARD), holds));
}

/// Whether the box is open and so has the keyboard, which is then not for the keys of actions.
/// The field has the keyboard too, but not in the pass in which the box first appears: egui
/// draws a new area unseen once to learn its size, and lets go of its widgets' focus.
pub fn holds_keyboard(ctx: &Context) -> bool {
    let held = ctx.data(|data| data.get_temp(Id::new(HOLDS_KEYBOARD)));
    held.unwrap_or(false)
}

impl VoxaApp {
    /// Show the search while it is open, and do what is chosen in it: a change of state goes
    /// onto `actions`, and what is asked of the window is done.
    pub(super) fn show_search(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        let Some(target) = self.search.show(ctx, self.session.state()) else {
            return;
        };
        if let Some(request) = run::run(&target, self.session.state(), actions) {
            self.ask(ctx, request);
        }
    }
}
