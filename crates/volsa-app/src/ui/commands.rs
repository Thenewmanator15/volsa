//! The list of every action a person can take, each described once: its name, the other
//! words it may be searched by, its key, where it lives, when it can be used and why not, and
//! what it does. The buttons of the bars, the keys and the list of keys in the help all read
//! it, so a name, a key or a hint is written in one place only.
//!
//! The entries themselves are in [`table`], a file for each part of the window.

use std::borrow::Cow;
use std::sync::LazyLock;

use egui::Key;

use super::keys::Held;
pub use super::panel::Tab;
use super::scene::export;
use super::tabs::Save;
use super::theme::ThemeChoice;
use super::top::Pick;
use crate::action::Action;
use crate::profile::Profile;
use crate::state::AppState;

mod draw;
pub mod menus;
mod request;
mod table;

#[cfg(test)]
pub use draw::spoken;
pub use draw::{button, button_labelled, button_made, button_made_noting, offer, offer_small};
#[cfg(test)]
pub use menus::MENUS;
pub use menus::{menu_button, named};
pub use request::{Confirm, Request, TagsOf};
pub(in crate::ui) use table::{no_3d_view, no_picture};

/// One action a person can take.
#[derive(Debug, Clone)]
pub struct Command {
    /// What the action is known by in the code, which never changes: `slice.after`.
    pub id: &'static str,
    /// What it does in plain words: what the help, search and a screen reader say of it.
    pub name: &'static str,
    /// What its button shows, which may be short: `1`, `◀`, `VR`, `A`.
    pub label: &'static str,
    /// More about it, shown under its name when the pointer rests on its button; empty when
    /// the name says enough.
    pub explains: &'static str,
    /// Other words a person may look for it by.
    pub words: &'static [&'static str],
    /// Its key, if it has one.
    pub key: Option<Shortcut>,
    /// Where in the window it lives.
    pub place: Place,
    /// The profiles of scan it is offered for; empty when it is offered for every scan.
    pub profiles: &'static [Profile],
    /// Why it cannot be used now, in a sentence; `None` when it can.
    pub why_not: fn(&AppState) -> Option<Why>,
    /// What it does.
    pub does: Does,
}

/// Why an action cannot be used now, in a sentence: most are written once, and some say what
/// the scan itself says, such as why it has no 3D view.
pub type Why = Cow<'static, str>;

/// A key, with the keys held with it, and how the help writes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    /// The key pressed.
    pub key: Key,
    /// Whether Ctrl, or Command on a Mac, is held with it.
    pub command: bool,
    /// Whether Shift is held with it.
    pub shift: bool,
    /// How the help and the hints write it: `Ctrl+Z`, `Page Up`.
    pub shown: &'static str,
}

/// Where in the window an action lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// The bar across the top of the window.
    TopBar,
    /// The menu of the top bar.
    Menu,
    /// The strip above the pictures that arranges them.
    Strip,
    /// The row of tabs of the panel at the right, and the button that folds it.
    Panel,
    /// A tab of the panel at the right.
    Tab(Tab),
    /// The rail of scans at the left: the strip it folds to, and its group of other files.
    Rail,
    /// The menu of a scan's card.
    Card,
    /// The line along the bottom of the window.
    Line,
    /// The About window.
    About,
    /// The document shown in the middle of the window.
    Document,
    /// Nowhere but its key.
    Keys,
}

impl Place {
    /// Where the action lives, in a few words for a person looking for it; `None` for one that has
    /// only its key.
    pub const fn said(self) -> Option<&'static str> {
        Some(match self {
            Self::TopBar => "In the top bar",
            Self::Menu => "In the menu at the right of the top bar",
            Self::Strip => "Above the pictures",
            Self::Panel => "In the tab panel",
            Self::Tab(Tab::View) => "In the View tab",
            Self::Tab(Tab::Measure) => "In the Measure tab",
            Self::Tab(Tab::Scene) => "In the 3D tab",
            Self::Tab(Tab::Labels) => "In the Labels tab",
            Self::Tab(Tab::Tools) => "In the Tools tab",
            Self::Rail => "Beside the scans",
            Self::Card => "In the menu of a scan's card",
            Self::Line => "In the line along the bottom",
            Self::About => "In the About window",
            Self::Document => "In the document",
            Self::Keys => return None,
        })
    }
}

/// What an action does.
#[derive(Debug, Clone)]
pub enum Does {
    /// A change of state, made from the state as it is; `None` when there is nothing to change.
    Act(fn(&AppState) -> Option<Action>),
    /// Something of the window, not of the state.
    Ask(Request),
}

impl Does {
    /// Do it in `state`: a change of state goes onto `actions`; what is asked of the window
    /// is given back.
    pub fn run(self, state: &AppState, actions: &mut Vec<Action>) -> Option<Request> {
        match self {
            Self::Act(act) => {
                actions.extend(act(state));
                None
            }
            // With a document open the pictures are out of sight: the file asked for is its.
            Self::Ask(Request::Tags(TagsOf::Shown)) if state.document.is_some() => {
                let entry = state.document.as_ref().map(|open| open.entry.clone());
                entry.map(|entry| Request::Tags(TagsOf::Entry(entry)))
            }
            Self::Ask(Request::SaveDocumentPdf(_)) => {
                let entry = state.document.as_ref().map(|open| open.entry.clone());
                entry.map(Request::SaveDocumentPdf)
            }
            Self::Ask(request) => Some(request),
        }
    }
}

/// Every entry, in the order the help lists their keys.
static ALL: LazyLock<Vec<Command>> = LazyLock::new(table::all);

/// Every action, in the order the help lists their keys.
pub fn all() -> &'static [Command] {
    &ALL
}

/// The action known by `id`.
pub fn find(id: &str) -> Option<&'static Command> {
    all().iter().find(|command| command.id == id)
}

/// Keys that do what another action's key does: Ctrl+Shift+Z redoes, as in many programs, and a
/// slash searches as Ctrl+K does, with Shift held or not: on a keyboard where a slash needs Shift
/// (a German one, Shift+7) it is still the slash, and a question mark, which egui reports as
/// its own key, shows the keys.
const OTHER_KEYS: [(Key, Held, &str); 3] = [
    (
        Key::Z,
        Held {
            command: true,
            shift: true,
        },
        "measure.redo",
    ),
    (
        Key::Slash,
        Held {
            command: false,
            shift: true,
        },
        "search.open",
    ),
    (
        Key::Slash,
        Held {
            command: false,
            shift: false,
        },
        "search.open",
    ),
];

/// The action a key asks for in `state`, if any. Ctrl must be held exactly when the key's
/// shortcut says; Shift too, except that a key without Shift is still taken with Shift held,
/// as Volsa always has. Ctrl+=, Ctrl+- and Ctrl+0 are left to egui, which sizes the interface.
pub fn for_key(key: Key, held: Held, state: &AppState) -> Option<&'static Command> {
    if left_to_egui(key, held) {
        return None;
    }
    let keyed = |shift: bool| {
        all().iter().find(|command| {
            command.key.is_some_and(|shortcut| {
                (shortcut.key, shortcut.command, shortcut.shift) == (key, held.command, shift)
            })
        })
    };
    let other = OTHER_KEYS
        .iter()
        .find(|(other, with, _)| (*other, *with) == (key, held))
        .and_then(|(_, _, id)| find(id));
    let found = other
        .or_else(|| keyed(held.shift))
        .or_else(|| keyed(false).filter(|_| held.shift))?;
    offered(found, state).then_some(found)
}

/// Whether egui itself answers this key, as it does the keys that size the interface.
fn left_to_egui(key: Key, held: Held) -> bool {
    use egui::gui_zoom::kb_shortcuts::{ZOOM_IN, ZOOM_IN_SECONDARY, ZOOM_OUT, ZOOM_RESET};
    let sizing = [ZOOM_IN, ZOOM_IN_SECONDARY, ZOOM_OUT, ZOOM_RESET];
    held.command && sizing.iter().any(|shortcut| shortcut.logical_key == key)
}

/// Whether an action is offered for the scan shown in the active viewport: an action for
/// some profiles only is not offered while no scan is shown.
pub fn offered(command: &Command, state: &AppState) -> bool {
    let shown = state.shown_info(state.active).map(Profile::of);
    command.profiles.is_empty() || shown.is_some_and(|profile| command.profiles.contains(&profile))
}

/// What an action's button says when the pointer rests on it: its name and key, as
/// `Name (Key)`, or its name alone, and beneath, what explains it.
pub fn hint(command: &Command) -> String {
    let mut hint = match command.key {
        Some(key) => format!("{} ({})", command.name, key.shown),
        None => command.name.to_owned(),
    };
    if !command.explains.is_empty() {
        hint.push('\n');
        hint.push_str(command.explains);
    }
    hint
}

#[cfg(test)]
mod button_tests;
#[cfg(test)]
mod table_tests;
#[cfg(test)]
mod tests;
