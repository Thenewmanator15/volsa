//! Drawing the button of an action: its words from the list, its hint, why it cannot be used
//! while it cannot, and its name for a screen reader, in one place.

use std::borrow::Cow;

use egui::{AtomLayoutResponse, Button, Ui};

use super::{Command, Does, Request, find, hint, offered};
use crate::action::Action;
use crate::state::AppState;

/// Draw the button of the action `id`, showing its label, and give what it does when it is
/// clicked. `selected` draws a button that stays pressed while it is `Some(true)`. While the
/// action cannot be used, the button is greyed and says why; for a scan it is not offered
/// for, nothing is drawn.
pub fn button(ui: &mut Ui, id: &str, state: &AppState, selected: Option<bool>) -> Option<Does> {
    let command = find(id)?;
    labelled(ui, command, command.label, state, selected)
}

/// Draw the button of the action `id` and take what it does: a change of state onto `actions`,
/// anything it asks of the window onto `asked`. Nothing is left undone.
pub fn offer(
    ui: &mut Ui,
    id: &str,
    state: &AppState,
    selected: Option<bool>,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    if let Some(does) = button(ui, id, state, selected) {
        asked.extend(does.run(state, actions));
    }
}

/// As [`offer`], with a small button, as the rows of a list and the lines of a tab have.
pub fn offer_small(
    ui: &mut Ui,
    id: &str,
    state: &AppState,
    selected: Option<bool>,
    (actions, asked): (&mut Vec<Action>, &mut Vec<Request>),
) {
    let Some(command) = find(id) else {
        return;
    };
    let label = command.label;
    let made = match selected {
        Some(on) => Button::selectable(on, label),
        None => Button::new(label),
    };
    if let Some((_, Some(does))) = draw(ui, command, (made.small(), label), (state, "")) {
        asked.extend(does.run(state, actions));
    }
}

/// As [`button`], with a label that follows the state, such as Play that becomes Pause.
pub fn button_labelled(
    ui: &mut Ui,
    id: &str,
    label: &str,
    state: &AppState,
    selected: Option<bool>,
) -> Option<Does> {
    labelled(ui, find(id)?, label, state, selected)
}

/// As [`button`], with the button itself made by `make` from the action, for a button that is
/// more than a label: one with its key beside its name, or an icon drawn on it. `shows` is
/// the words the made button writes on itself, for a screen reader to say with the action's
/// name (see [`spoken`]); empty when the button writes the name itself or only a picture.
/// Gives where the button was drawn, with the places of its custom atoms, and what it does
/// if it was clicked; `None` when the action is not offered for the scan shown.
pub fn button_made<'a>(
    ui: &mut Ui,
    id: &str,
    state: &AppState,
    shows: &str,
    make: impl FnOnce(&Command) -> Button<'a>,
) -> Option<(AtomLayoutResponse, Option<Does>)> {
    let command = find(id)?;
    draw(ui, command, (make(command), shows), (state, ""))
}

/// As [`button_made`], with a note beneath the hint when the pointer rests on the button: what
/// the action cannot know, such as the words of the notice that its button shows.
pub fn button_made_noting<'a>(
    ui: &mut Ui,
    id: &str,
    state: &AppState,
    (shows, note): (&str, &str),
    make: impl FnOnce(&Command) -> Button<'a>,
) -> Option<(AtomLayoutResponse, Option<Does>)> {
    let command = find(id)?;
    draw(ui, command, (make(command), shows), (state, note))
}

/// What a screen reader calls the button of `command` that writes `shows` on itself: the
/// action's name when that holds the words (whatever their case), so that a person who
/// speaks the words they can see is understood (WCAG 2.5.3, label in name); otherwise the
/// words, then the name, as `1, One viewport`. A mark with no letter or digit in it, such as
/// `◀` or `?`, is a picture and not words anyone speaks, so the name stands alone. The help
/// and the search say the name alone.
pub fn spoken(command: &Command, shows: &str) -> Cow<'static, str> {
    let shows = shows.trim();
    let words = shows.chars().any(char::is_alphanumeric);
    let holds = command.name.to_lowercase().contains(&shows.to_lowercase());
    if !words || holds {
        Cow::Borrowed(command.name)
    } else {
        Cow::Owned(format!("{shows}, {}", command.name))
    }
}

/// Draw an action's button with this label.
fn labelled(
    ui: &mut Ui,
    command: &Command,
    label: &str,
    state: &AppState,
    selected: Option<bool>,
) -> Option<Does> {
    let button = match selected {
        Some(on) => Button::selectable(on, label),
        None => Button::new(label),
    };
    draw(ui, command, (button, label), (state, ""))?.1
}

/// Draw an action's button, which writes `written` on itself: its hint with `note` beneath it, why
/// not while it cannot be used, and its name for a screen reader, whatever the button says.
fn draw(
    ui: &mut Ui,
    command: &Command,
    (button, written): (Button<'_>, &str),
    (state, note): (&AppState, &str),
) -> Option<(AtomLayoutResponse, Option<Does>)> {
    if !offered(command, state) {
        return None;
    }
    let why_not = (command.why_not)(state);
    let mut shown = if why_not.is_none() {
        button.atom_ui(ui)
    } else {
        ui.add_enabled_ui(false, |ui| button.atom_ui(ui)).inner
    };
    // A screen reader says the name, which a short label such as `◀` does not, after the
    // label if the name does not hold it.
    let said = spoken(command, written);
    ui.ctx()
        .accesskit_node_builder(shown.response.id, |node| node.set_label(said));
    let mut hint = hint(command);
    if !note.is_empty() {
        hint.push('\n');
        hint.push_str(note);
    }
    shown.response = shown.response.on_hover_text(hint);
    if let Some(why) = why_not {
        shown.response = shown.response.on_disabled_hover_text(why);
    }
    let does = shown.response.clicked().then(|| command.does.clone());
    Some((shown, does))
}
