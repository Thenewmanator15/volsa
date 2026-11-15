//! The Settings window: what is kept for every scan from one run to the next. The theme, as
//! a choice of three; the interface size, with the keys that change it from anywhere; and which
//! side of the screen the patient's left is drawn on. It floats over the pictures as About and
//! the help do, opens from the top bar's menu or by Ctrl+, and closes with Esc.
//!
//! Each of its buttons is an action of the list of actions, as the top bar's and the View
//! tab's are, so it says nothing of its own that the list does not.

use egui::{Context, Id, RichText, Ui, Window};

use super::commands::{self, Request};
use super::theme::{self, ThemeChoice};
use super::{floating, size};
use crate::action::Action;
use crate::state::AppState;

/// The window's title.
pub const TITLE: &str = "Settings";
/// The heading of the choice of theme.
pub const THEME: &str = "Theme";
/// The heading of the choice of interface size.
pub const SIZE: &str = "Interface size";
/// The heading of the switch of which side the patient's left is drawn on.
pub const SIDES: &str = "Left and right";
/// The action of the switch of which side the patient's left is drawn on.
pub const RADIOLOGICAL: &str = "view.radiological";

/// The interface's own note, kept between frames, that the window is open.
fn is_open_id() -> Id {
    Id::new("volsa: the settings window is open")
}

/// Open the window: what its action asks.
pub fn open(ctx: &Context) {
    ctx.data_mut(|data| data.insert_temp(is_open_id(), true));
}

/// Close the window, which Esc does when it was opened last.
pub fn close(ctx: &Context) {
    ctx.data_mut(|data| data.insert_temp(is_open_id(), false));
}

/// Whether the window is open.
pub fn is_open(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp(is_open_id()))
        .unwrap_or(false)
}

/// Show the window while it is open, for the window drawn in `theme`: a change of state goes
/// onto `actions`, and what is asked of the window onto `asked`.
pub fn show(
    ctx: &Context,
    state: &AppState,
    theme: ThemeChoice,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    let mut open = is_open(ctx);
    if !open {
        return;
    }
    let window = Window::new(TITLE)
        .collapsible(false)
        .resizable(false)
        .open(&mut open);
    floating::over(window, ctx).show(ctx, |ui| {
        contents(ui, state, theme, (actions, asked));
    });
    if !open {
        close(ctx);
    }
}

/// What the window holds, a heading over each choice.
fn contents(
    ui: &mut Ui,
    state: &AppState,
    theme: ThemeChoice,
    (actions, asked): (&mut Vec<Action>, &mut Vec<Request>),
) {
    heading(ui, THEME);
    ui.horizontal_wrapped(|ui| {
        for choice in ThemeChoice::ALL {
            let on = Some(choice == theme);
            commands::offer(ui, choice.command(), state, on, actions, asked);
        }
    });
    ui.add_space(8.0);
    heading(ui, SIZE);
    asked.extend(size::choices(ui, state).map(Request::Size));
    ui.add_space(8.0);
    heading(ui, SIDES);
    let on = Some(state.radiological);
    commands::offer(ui, RADIOLOGICAL, state, on, actions, asked);
    let quiet = theme::palette(ui.ctx()).quiet_text;
    let note = "The View tab has this switch too.";
    ui.add(egui::Label::new(RichText::new(note).small().color(quiet)).wrap());
}

/// A heading of the window.
fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).strong());
}

#[cfg(test)]
mod tests;
