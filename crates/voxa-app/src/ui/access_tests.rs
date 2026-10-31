//! What a screen reader is told of the window: every control and picture has a name, the names
//! of the buttons of the bars and the tabs are those of the list of actions, each card and
//! viewport says what it shows, and the rail is walked with the arrow keys.

mod names;
mod rail_keys;
mod window;

use egui::accesskit::Role;
use voxa_dicom::{Report, ReportLine};

use self::window::{Heard, Told, Window};
use super::commands;
use super::panel::Tab;
use super::tabs::made_up::labelled_by;
use super::top;
use crate::action::Action;
use crate::annotations::Shape;
use crate::labels::NamesFrom;
use crate::pictures::Pictures;
use crate::reducer::reduce;
use crate::state::{AppState, Layout, OpenDocument};
use crate::testing::documents::{REPORT, scans_and_documents};
use crate::testing::thick_scan_ready;
use crate::views::ViewsAction;

/// A scan with labels and a ruler and a marker measured on it, in four viewports, and a saved
/// view of its scene.
fn measured_in_four() -> AppState {
    let mut state = labelled_by(None, NamesFrom::Model, None);
    let ruler = Shape::Ruler {
        a: [0.0, 0.0, 10.0],
        b: [3.0, -4.0, 10.0],
    };
    let marker = Shape::Marker {
        at: [-12.0, 4.5, 30.25],
    };
    for shape in [ruler, marker] {
        let _ = reduce(&mut state, Action::AnnotationAdd(shape));
    }
    let save = ViewsAction::Save {
        name: "Front".to_owned(),
    };
    let _ = reduce(&mut state, Action::Views(save));
    let _ = reduce(&mut state, Action::SetLayout(Layout::TwoByTwo));
    state
}

/// `state` with these pictures arranged, a cut in the scene and the shown slice drawn in it.
fn arranged(mut state: AppState, pictures: Pictures) -> AppState {
    let _ = reduce(&mut state, Action::SetPictures(pictures));
    let _ = reduce(&mut state, Action::CutAdd);
    let _ = reduce(&mut state, Action::ShowSliceInScene(true));
    state
}

/// A report open in the middle of the window.
fn with_a_report() -> AppState {
    let mut state = thick_scan_ready();
    let _ = reduce(&mut state, Action::IndexProgress(scans_and_documents()));
    let line = |depth, text: &str| ReportLine {
        depth,
        heading: None,
        text: text.to_owned(),
        is_name: false,
    };
    let lines = vec![line(0, "Part A"), line(1, "Width: 12 mm")];
    let report = Report {
        lines,
        cut_short: false,
    };
    let entry = REPORT.to_owned();
    state.document = Some(OpenDocument {
        entry,
        report: Some(Ok(report)),
    });
    state
}

/// The window in each of its arrangements and with a report open, with a name for each.
fn every_window() -> Vec<(&'static str, AppState)> {
    vec![
        ("four slices", measured_in_four()),
        (
            "slices and 3D",
            arranged(measured_in_four(), Pictures::SlicesAndScene),
        ),
        ("planes", arranged(measured_in_four(), Pictures::Planes)),
        ("3D alone", arranged(measured_in_four(), Pictures::Scene)),
        ("a report", with_a_report()),
    ]
}

/// The window for `state` with this tab open, as a screen reader is told of it.
fn told_of(state: &AppState, tab: Tab) -> (Told, Window) {
    let mut window = Window::new();
    window.panel.tab = tab;
    let told = window.told(state);
    (told, window)
}

/// Whether a screen reader must be able to say what the node is: anything the keyboard can
/// reach or a click can press, and any button, switch, tab or list item. Text is not a
/// control: a label's words are what a screen reader reads of it.
fn needs_a_name(node: &Heard) -> bool {
    let roles = [Role::Button, Role::CheckBox, Role::Tab, Role::ListItem];
    let text = matches!(node.role, Role::Label | Role::TextRun);
    !node.hidden && !text && (node.focusable || node.clickable || roles.contains(&node.role))
}

#[test]
fn every_interactive_node_has_a_name() {
    for (window, state) in every_window() {
        for tab in Tab::ALL {
            let (told, _) = told_of(&state, tab);
            let unnamed: Vec<&Heard> = told
                .nodes
                .iter()
                .filter(|node| needs_a_name(node) && node.name.trim().is_empty())
                .collect();
            assert!(unnamed.is_empty(), "{window}, {tab:?}: {unnamed:#?}");
            // A field that says something before anything is typed is named by what it says.
            let fields = told
                .nodes
                .iter()
                .filter(|node| !node.placeholder.is_empty());
            for field in fields {
                let holds = field
                    .name
                    .to_lowercase()
                    .contains(&field.placeholder.to_lowercase());
                assert!(
                    holds,
                    "{window}, {tab:?}: {field:?} is not named by what it says"
                );
            }
            // The bars and the tabs hold plenty of buttons, or the check means nothing.
            assert!(told.of(Role::Button).len() > 10, "{window}, {tab:?}");
        }
    }
}

/// Whether a button's name is one the list of actions gives: the action's name, or the name
/// after the label that was written on the button, as `4, Four viewports`.
fn is_an_actions(name: &str) -> bool {
    let search = commands::find("search.open").expect("listed");
    let as_the_box_writes = commands::spoken(search, top::SEARCH_SAYS);
    let of_an_action = |command: &commands::Command| {
        name == command.name || name == commands::spoken(command, command.label)
    };
    commands::all().iter().any(of_an_action) || name == as_the_box_writes
}

/// The buttons that are not actions of the list, called by these names: the menus, which open
/// more actions; the Labels tab's own switches, with the three ways labels are drawn over the
/// planes; and the Tools tab's buttons. A value field (the window, the level, the speed), the
/// switch Loop and the switch of each side of a part are not buttons, so need no place here.
const BY_NAME: [&str; 12] = [
    "Open",
    "More",
    "Views",
    "Export",
    "Options",
    "Show all",
    "Close",
    "Names…",
    "Save as CSV…",
    "Off",
    "Outline",
    "Filled",
];

/// The buttons that are not actions of the list, called by names that begin with these words:
/// the rows of lists, each of which belongs to one item and so is not an action. A
/// measurement's Remove and Go to, a saved view's Delete, a group of parts' Show or Hide, and
/// the report's own Close and Save a copy.
const BY_ROW: [&str; 6] = [
    "Remove ",
    "Go to ",
    "Delete ",
    "Show ",
    "Hide ",
    "Save a copy as PDF",
];

#[test]
fn every_button_in_the_bars_and_tabs_is_a_command() {
    for (window, state) in every_window() {
        for tab in Tab::ALL {
            let (told, hand) = told_of(&state, tab);
            let outside_the_rail = told
                .nodes
                .iter()
                .filter(|node| node.at.is_some_and(|at| !hand.rail_at.contains_rect(at)));
            let buttons = outside_the_rail.filter(|node| node.role == Role::Button && !node.hidden);
            for button in buttons {
                let name = button.name.as_str();
                let by_hand =
                    BY_NAME.contains(&name) || BY_ROW.iter().any(|row| name.starts_with(row));
                // A saved view and a part are each a button named by what they are.
                let a_view = state.views.iter().any(|view| view.name == name);
                let a_part = state.labels.parts.iter().any(|part| part.name == name);
                assert!(
                    is_an_actions(name) || by_hand || a_view || a_part,
                    "{window}, {tab:?}: {name:?} is not an action's name"
                );
            }
        }
    }
}
