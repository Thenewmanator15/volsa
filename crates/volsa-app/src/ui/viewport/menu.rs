//! The menu a slice opens when it is clicked with the other button and not dragged: dragging
//! with that button sets the window, as it always has. Each item is an action of the list of
//! actions, so the menu, the View tab, the keys and the search do the same thing.

use egui::{Response, Ui};

use crate::action::Action;
use crate::state::AppState;
use crate::ui::commands::{self, Request};

/// The menu's items, in order: the action each is, and the words it shows.
pub(super) const ITEMS: [(&str, &str); 8] = [
    ("view.fit", "Fit"),
    ("view.flip_across", "Mirror across"),
    ("view.flip_down", "Mirror down"),
    ("view.turn", "Turn"),
    ("view.reset", "Reset view"),
    ("window.auto", "Auto brightness"),
    ("view.save_slice", "Save this slice as a picture…"),
    ("tags.show", "Show tags"),
];

/// After which items a line is drawn: between the view, the brightness, and the file.
const LINE_AFTER: [&str; 2] = ["view.reset", "window.auto"];

/// Open the menu of the viewport `response` is of when it is clicked with the other button, and
/// take what is chosen from it: a change of state onto `actions`, what it asks of the window
/// onto `asked`. The press that opened it made that viewport the active one, which the actions
/// act on.
pub(super) fn offer(
    response: &Response,
    state: &AppState,
    actions: &mut Vec<Action>,
    asked: &mut Vec<Request>,
) {
    response.context_menu(|ui| items(ui, state, actions, asked));
}

/// The items of the menu, each closing it once chosen.
fn items(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    for (id, words) in ITEMS {
        if let Some(does) = commands::button_labelled(ui, id, words, state, None) {
            asked.extend(does.run(state, actions));
            ui.close();
        }
        if LINE_AFTER.contains(&id) {
            ui.separator();
        }
    }
}
