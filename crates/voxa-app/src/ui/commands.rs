//! The list of every action a person can take, each described once: its name, the other
//! words it may be searched by, its key, where it lives, when it can be used and why not, and
//! what it does. The buttons of the bars, the keys and the list of keys in the help all read
//! it, so a name, a key or a hint is written in one place only.
//!
//! The entries themselves are in [`table`], a file for each part of the window.

use std::borrow::Cow;
use std::sync::LazyLock;

use egui::{AtomLayoutResponse, Button, Key, Ui};

use super::keys::Held;
pub use super::panel::Tab;
use super::parts::Asked;
use super::scene::export;
use super::tabs::Save;
use super::theme::ThemeChoice;
use super::top::Pick;
use crate::action::Action;
use crate::profile::Profile;
use crate::state::AppState;

mod table;

/// One action a person can take.
#[derive(Debug, Clone, Copy)]
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the panel, the strip and the top bar read it (Tasks 6 to 10)"
        )
    )]
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
    /// The card of a scan.
    #[expect(
        dead_code,
        reason = "the tags' action is in the top bar's menu until Task 11 gives the cards one"
    )]
    Card,
    /// Nowhere but its key.
    Keys,
}

/// What an action does.
#[derive(Debug, Clone, Copy)]
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
            Self::Ask(request) => Some(request),
        }
    }
}

/// What a person asks of the window rather than of the scan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Request {
    /// To choose a scan to open, of this kind.
    Open(Pick),
    /// To search the actions, parts, scans and help.
    Search,
    /// To open or close the help, at a topic if one is named.
    Help(Option<&'static str>),
    /// To see the About window.
    About,
    /// To open or close the tags of the file shown.
    Tags,
    /// To go into full screen, or out of it.
    FullScreen,
    /// To show a tab of the panel.
    ShowTab(Tab),
    /// To fold the column of scans, or unfold it.
    FoldRail,
    /// To fold the panel of tabs, or unfold it.
    FoldPanel,
    /// To draw the window in this theme.
    Theme(ThemeChoice),
    /// To draw the interface at this size, 1 being its usual size.
    Size(f32),
    /// To draw the interface a step larger.
    Larger,
    /// To draw the interface a step smaller.
    Smaller,
    /// To save the measurements as this kind of file.
    SaveMeasurements(Save),
    /// To save the 3D scene as a picture or a turntable.
    SaveScene(export::Asked),
    /// To open a file of labels.
    OpenLabels,
    /// To choose a file the labels or their labelling need, or where to save them.
    Choose(Asked),
    /// To label the scan in the scene with Voxa's own networks, told how much of the graphics
    /// card the window holds itself, which only the window can say.
    LabelScan,
    /// To say, in a notice, where the log of this run is.
    WhereTheLogIs,
    /// To show the files that are not scans.
    #[expect(dead_code, reason = "Task 11 adds the other files")]
    ShowOtherFiles,
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

/// Keys that do what another action's key does: Ctrl+Shift+Z redoes, as in many programs, a
/// slash searches as Ctrl+K does, and a question mark, which is Shift with slash on some
/// keyboards, shows the keys.
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
        "help.keys",
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
/// as Voxa always has. Ctrl+=, Ctrl+- and Ctrl+0 are left to egui, which sizes the interface.
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
    let shown = state
        .viewports
        .get(state.active)
        .and_then(|viewport| viewport.series.as_ref())
        .and_then(|series| state.series(series))
        .map(|listed| Profile::of(&listed.info));
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

/// Draw the button of the action `id`, showing its label, and give what it does when it is
/// clicked. `selected` draws a button that stays pressed while it is `Some(true)`. While the
/// action cannot be used, the button is greyed and says why; for a scan it is not offered
/// for, nothing is drawn.
pub fn button(ui: &mut Ui, id: &str, state: &AppState, selected: Option<bool>) -> Option<Does> {
    let command = find(id)?;
    labelled(ui, command, command.label, state, selected)
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
/// more than a label: one with its key beside its name, or an icon drawn on it. Gives where
/// the button was drawn, with the places of its custom atoms, and what it does if it was
/// clicked; `None` when the action is not offered for the scan shown.
pub fn button_made<'a>(
    ui: &mut Ui,
    id: &str,
    state: &AppState,
    make: impl FnOnce(&Command) -> Button<'a>,
) -> Option<(AtomLayoutResponse, Option<Does>)> {
    let command = find(id)?;
    draw(ui, command, make(command), state)
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
    draw(ui, command, button, state)?.1
}

/// Draw an action's button: its hint, why not while it cannot be used, and its name for a
/// screen reader, whatever the button says.
fn draw(
    ui: &mut Ui,
    command: &Command,
    button: Button<'_>,
    state: &AppState,
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
    // A screen reader says the name, which a short label such as `◀` does not.
    ui.ctx()
        .accesskit_node_builder(shown.response.id, |node| node.set_label(command.name));
    shown.response = shown.response.on_hover_text(hint(command));
    if let Some(why) = why_not {
        shown.response = shown.response.on_disabled_hover_text(why);
    }
    let does = shown.response.clicked().then_some(command.does);
    Some((shown, does))
}

#[cfg(test)]
mod button_tests;
#[cfg(test)]
mod table_tests;
#[cfg(test)]
mod tests;
