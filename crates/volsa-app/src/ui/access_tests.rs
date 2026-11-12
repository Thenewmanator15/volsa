//! What a screen reader is told of the window: every control and picture has a name, the names
//! of the buttons of the bars and the tabs are those of the list of actions, each card and
//! viewport says what it shows, and the rail is walked with the arrow keys.

mod names;
mod rail_keys;
mod walk;
mod window;

use egui::accesskit::Role;
use volsa_dicom::{Report, ReportLine};

use self::walk::{Scene, check_buttons, extra_windows};
use self::window::{Heard, Told, Window};
use super::commands;
use super::panel::Tab;
use super::tabs::made_up::labelled_by;
use crate::action::Action;
use crate::annotations::Shape;
use crate::labels::NamesFrom;
use crate::pictures::Pictures;
use crate::reducer::reduce;
use crate::state::{AppState, Layout, OpenDocument};
use crate::testing::documents::{PDF, REPORT, scans_and_documents};
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

/// A PDF open in the middle of the window: it is not shown, and a copy of it can be saved.
fn with_a_pdf() -> AppState {
    let mut state = thick_scan_ready();
    let _ = reduce(&mut state, Action::IndexProgress(scans_and_documents()));
    let entry = PDF.to_owned();
    state.document = Some(OpenDocument {
        entry,
        report: None,
    });
    state
}

/// The window in each of its arrangements and with a report or a PDF open, with a name for each.
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
        ("a PDF", with_a_pdf()),
    ]
}

/// Every state of the window the walks read: the arrangements and the report, and the states
/// with a labelling run, a kept result, labels that Volsa made, a notice, a floating window, a
/// question or a menu open.
fn every_scene() -> Vec<Scene> {
    let plain = every_window()
        .into_iter()
        .map(|(name, state)| Scene::plain(name, state));
    plain.chain(extra_windows()).collect()
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
    for scene in every_scene() {
        let window = scene.name;
        for tab in Tab::ALL {
            let (told, _) = walk::told_in(&scene, tab);
            // What egui makes of a popup, which holds the controls that are named, and of the title
            // bar of a window that floats, which lies in the window, and a scroll bar, are not
            // controls of Volsa's to name.
            let egui_s = |node: &Heard| {
                let holds = |other: &Heard| {
                    let (outer, inner) = (node.at, other.at);
                    let named = !other.name.is_empty() && other.id != node.id;
                    named && outer.zip(inner).is_some_and(|(o, i)| o.contains_rect(i))
                };
                let in_a_window = |other: &Heard| {
                    let (outer, inner) = (other.at, node.at);
                    let window = other.role == Role::Window;
                    window && outer.zip(inner).is_some_and(|(o, i)| o.contains_rect(i))
                };
                match node.role {
                    Role::ScrollBar => true,
                    Role::Unknown => told.nodes.iter().any(|one| holds(one) || in_a_window(one)),
                    _ => false,
                }
            };
            let unnamed: Vec<&Heard> = told
                .nodes
                .iter()
                .filter(|node| needs_a_name(node) && node.name.trim().is_empty() && !egui_s(node))
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

#[test]
fn every_button_in_the_bars_and_tabs_is_a_command_a_menu_or_a_row_named_by_its_item() {
    for scene in every_scene() {
        for tab in Tab::ALL {
            check_buttons(&scene, tab);
        }
    }
}

#[test]
fn the_states_walked_reach_the_buttons_that_used_to_be_drawn_by_hand() {
    // Each of these is only drawn in one state, so the walk sees it only if it reaches that state.
    let scenes = every_scene();
    let seen = |scene: &str, tab: Tab, name: &str| {
        let scene = scenes.iter().find(|one| one.name == scene).expect(scene);
        let (told, _) = walk::told_in(scene, tab);
        // By the name of the action, or by it after the label the button writes on itself.
        let command = commands::all().iter().find(|command| command.name == name);
        let spoken = command.map(|command| commands::spoken(command, command.label));
        let found = told
            .nodes
            .iter()
            .any(|node| node.name == name || spoken.as_deref() == Some(node.name.as_str()));
        assert!(found, "{} has no button {name:?} in {tab:?}", scene.name);
    };
    seen("a labelling run", Tab::Tools, "Stop labelling the scan");
    seen(
        "a kept result",
        Tab::Tools,
        "Show the labels kept for this series",
    );
    seen("a kept result", Tab::Tools, "Label this scan again");
    seen(
        "a kept result",
        Tab::Tools,
        "Delete the labels kept for this series",
    );
    seen(
        "labels that Volsa made",
        Tab::Labels,
        "Save the labels as a file",
    );
    seen("labels that Volsa made", Tab::Labels, "Close the labels");
    seen(
        "the notices listed",
        Tab::View,
        "Dismiss: Made-up: a slice could not be read",
    );
    seen("About", Tab::View, "Show the licences and notices");
    seen(
        "the labelling options menu",
        Tab::Tools,
        "Choose the folder of the models",
    );
    seen(
        "the labelling options menu",
        Tab::Tools,
        "Label the cortex areas",
    );
    seen("the Views menu", Tab::Scene, "Delete Front");
    seen(
        "the interface size menu",
        Tab::View,
        "Draw the interface at 150%",
    );
    seen("a question", Tab::View, "Keep them");
}

#[test]
fn an_empty_window_says_how_to_begin_in_the_middle_where_a_person_looks() {
    let scenes = every_scene();
    let empty = scenes
        .iter()
        .find(|one| one.name == "an empty window")
        .expect("walked");
    let (told, _) = walk::told_in(empty, Tab::View);
    for id in ["open.folder", "open.file"] {
        let command = commands::find(id).expect("listed");
        let name = commands::spoken(command, command.label);
        let button = told.called(&name).unwrap_or_else(|| panic!("{name}"));
        // Not the rail at the left, nor the bar across the top: in the middle.
        let at = button.at.expect("placed").center();
        let (left, right) = (window::RAIL, window::SIZE[0] - window::PANEL);
        assert!(
            at.x > left && at.x < right && at.y > 60.0,
            "{name} at {at:?}"
        );
    }
}
