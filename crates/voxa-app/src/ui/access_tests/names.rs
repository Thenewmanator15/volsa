//! The names a screen reader is given to what is drawn by hand: the cards of the rail, the
//! viewports, the 3D scene and the planes with their handles, the rows of a list, and the
//! search box.

use egui::accesskit::Role;
use voxa_core::{Facing, Plane, SeriesProblem};

use super::window::settled;
use super::window::{Heard, Told, Window};
use super::{measured_in_four, told_of, with_a_report};
use crate::action::Action;
use crate::pictures::Pictures;
use crate::reducer::reduce;
use crate::state::{AppState, Layout};
use crate::testing::{FINE_SCAN, SCOUT, id, thick_scan_ready};
use crate::ui::commands;
use crate::ui::panel::Tab;
use crate::ui::scene::{export, views};
use crate::ui::search::Search;
use crate::ui::{seen, top};

/// The cards of the rail as a screen reader is told of them, from the top down: the buttons
/// in the rail but for the one that opens the menu of each and the one at its top that folds it.
fn cards(told: &Told, window: &Window) -> Vec<String> {
    let folds = commands::find("rail.fold").map_or("", |command| command.name);
    let in_the_rail = |node: &&Heard| node.at.is_some_and(|at| window.rail_at.contains_rect(at));
    let mut found: Vec<&Heard> = told.nodes.iter().filter(in_the_rail).collect();
    found.retain(|node| {
        let a_card = node.role == Role::Button && !node.name.starts_with("More for ");
        a_card && node.name != folds
    });
    let top = |node: &Heard| node.at.map_or(0.0, |at| at.top());
    found.sort_by(|a, b| top(a).total_cmp(&top(b)));
    found.into_iter().map(|node| node.name.clone()).collect()
}

/// Give the series `series` this problem, as the rail and everything else reads it.
fn faulted(state: &mut AppState, series: &str, problem: &SeriesProblem) {
    let index = state.index.as_mut().expect("an export is open");
    let listed = index.series.iter_mut().map(|listed| &mut listed.info);
    let in_studies = index.studies.iter_mut().flat_map(|study| &mut study.series);
    for info in listed
        .chain(in_studies)
        .filter(|info| info.id == id(series))
    {
        info.problems = vec![problem.clone()];
    }
}

#[test]
fn a_card_says_its_name_and_whether_it_is_selected() {
    let mut state = thick_scan_ready();
    faulted(&mut state, SCOUT, &SeriesProblem::UnevenSpacing);
    let (told, window) = told_of(&state, Tab::View);
    // The one the viewport shows is selected, and the one that cannot be shown in full says so.
    assert_eq!(
        cards(&told, &window),
        [
            "Axial scan, 30 slices · 5 mm, selected",
            "Axial scan, 64 slices · 5 mm",
            "Axial scan, 3 slices · 5 mm, has a problem",
        ]
    );
    // Showing another series moves the word.
    let viewport = 0;
    let series = id(FINE_SCAN);
    reduce(&mut state, Action::SelectSeries { viewport, series });
    faulted(&mut state, FINE_SCAN, &SeriesProblem::UnevenSpacing);
    let (told, window) = told_of(&state, Tab::View);
    assert_eq!(
        cards(&told, &window),
        [
            "Axial scan, 30 slices · 5 mm",
            "Axial scan, 64 slices · 5 mm, selected, has a problem",
            "Axial scan, 3 slices · 5 mm, has a problem",
        ]
    );
    // The mark on the picture does not say it a second time, and the button of each card's
    // menu says whose it is.
    let marks = told
        .nodes
        .iter()
        .filter(|node| node.name == "has a problem");
    assert!(marks.clone().count() > 0 && marks.into_iter().all(|mark| mark.hidden));
    let more: Vec<&str> = told.nodes.iter().map(|node| node.name.as_str()).collect();
    assert!(more.contains(&"More for Axial scan, 3 slices · 5 mm"));
}

/// The pictures of the window as a screen reader is told of them, in no order.
fn pictures(told: &Told) -> Vec<&str> {
    let clickable = told
        .of(Role::Image)
        .into_iter()
        .filter(|node| node.clickable);
    clickable.map(|node| node.name.as_str()).collect()
}

#[test]
fn a_viewport_says_which_slice_it_shows() {
    let mut state = thick_scan_ready();
    // The files of the thick scan place it in the patient, so its plane is known.
    let (told, _) = told_of(&state, Tab::View);
    assert_eq!(pictures(&told), ["Axial slice 16 of 30, Axial scan"]);
    // Another slice is another number.
    let _ = reduce(
        &mut state,
        Action::SetSlice {
            viewport: 0,
            slice: 3,
        },
    );
    let (told, _) = told_of(&state, Tab::View);
    assert_eq!(pictures(&told), ["Axial slice 4 of 30, Axial scan"]);
    // A scan whose plane is not known has a slice and no plane; a tipped one says so.
    let plane = |plane| {
        let mut state = state.clone();
        let index = state.index.as_mut().expect("an export is open");
        for listed in &mut index.series {
            listed.info.scan.plane = plane;
        }
        pictures_of(&state)
    };
    assert_eq!(plane(None), ["Slice 4 of 30, Scan 1"]);
    let tipped = Plane {
        nearest: Facing::Coronal,
        oblique: true,
    };
    let said = plane(Some(tipped));
    assert_eq!(said.len(), 1);
    assert!(
        said[0].starts_with("Oblique coronal slice 4 of 30, "),
        "{said:?}"
    );
}

/// The pictures the window draws for `state`, as told.
fn pictures_of(state: &AppState) -> Vec<String> {
    let (told, _) = told_of(state, Tab::View);
    pictures(&told).into_iter().map(str::to_owned).collect()
}

#[test]
fn several_viewports_say_which_one_the_keys_act_on_and_which_are_empty() {
    let mut state = thick_scan_ready();
    let _ = reduce(&mut state, Action::SetLayout(Layout::TwoByTwo));
    let mut said = pictures_of(&state);
    said.sort();
    assert_eq!(
        said,
        [
            "Axial slice 16 of 30, Axial scan, selected",
            "No series is shown here",
            "No series is shown here",
            "No series is shown here",
        ]
    );
    let _ = reduce(&mut state, Action::SetActive(1));
    // An empty viewport can be the one the keys act on, too.
    let mut said = pictures_of(&state);
    said.sort();
    assert_eq!(
        said,
        [
            "Axial slice 16 of 30, Axial scan",
            "No series is shown here",
            "No series is shown here",
            "No series is shown here, selected",
        ]
    );
}

#[test]
fn the_3d_scene_and_the_planes_say_what_they_show() {
    let mut state = thick_scan_ready();
    let _ = reduce(&mut state, Action::SetPictures(Pictures::Scene));
    assert_eq!(pictures_of(&state), ["3D view of Axial scan"]);
    let _ = reduce(&mut state, Action::SetPictures(Pictures::Planes));
    let mut said = pictures_of(&state);
    said.sort();
    assert_eq!(
        said,
        [
            "3D view of Axial scan",
            "Axial plane, Axial scan",
            "Coronal plane, Axial scan",
            "Sagittal plane, Axial scan",
        ]
    );
}

#[test]
fn the_handles_on_the_scene_are_named_for_what_they_move() {
    let mut state = thick_scan_ready();
    let _ = reduce(&mut state, Action::SetPictures(Pictures::Planes));
    let _ = reduce(&mut state, Action::CutAdd);
    let _ = reduce(&mut state, Action::ShowSliceInScene(true));
    let (told, _) = told_of(&state, Tab::View);
    let handles: Vec<&str> = told
        .nodes
        .iter()
        .filter(|node| node.focusable && node.role == Role::Unknown)
        .map(|node| node.name.as_str())
        .collect();
    for wanted in ["Move cut 1", "Turn cut 1", "Move through the slices"] {
        assert!(handles.contains(&wanted), "{wanted} is not in {handles:?}");
    }
    let planes = handles
        .iter()
        .filter(|name| name.ends_with("plane through its layers"));
    assert!(planes.count() >= 2, "{handles:?}");
}

#[test]
fn each_measurement_is_named_in_each_of_its_controls() {
    let state = measured_in_four();
    let labels: Vec<&str> = state
        .annotations
        .items
        .iter()
        .map(|item| item.label.as_str())
        .collect();
    assert_eq!(labels, ["Ruler 1", "Marker 2"]);
    let (told, _) = told_of(&state, Tab::Measure);
    let named = |name: &str| told.nodes.iter().filter(|node| node.name == name).count();
    for label in labels {
        assert_eq!(named(&format!("Remove {label}")), 1, "{label}");
        assert_eq!(named(&format!("Name of {label}")), 1, "{label}");
        let go_to = told
            .nodes
            .iter()
            .filter(|node| node.name.starts_with(&format!("Go to {label}, ")));
        assert_eq!(go_to.count(), 1, "{label}");
    }
    // The name field is a field, and is called what it names, not what is typed in it.
    let field = told.called("Name of Ruler 1").expect("the field");
    assert_eq!(field.role, Role::TextInput);
    assert_eq!(field.value, "Ruler 1");
}

#[test]
fn the_search_box_is_named_by_the_words_it_shows() {
    let (told, _) = told_of(&thick_scan_ready(), Tab::View);
    let search = told
        .nodes
        .iter()
        .find(|node| node.name.starts_with(top::SEARCH_SAYS))
        .expect("the box");
    assert!(
        search.name.contains("Find a tool or part"),
        "{}",
        search.name
    );
    assert_eq!(search.role, Role::Button);
}

#[test]
fn the_fields_of_the_view_tab_say_what_they_set() {
    let (told, _) = told_of(&thick_scan_ready(), Tab::View);
    for wanted in ["Window width", "Window level", "Playback speed"] {
        let field = told.called(wanted).unwrap_or_else(|| panic!("{wanted}"));
        assert_eq!(field.role, Role::SpinButton, "{wanted}");
    }
}

#[test]
fn the_menus_name_what_they_hold() {
    let state = measured_in_four();
    let ctx = seen::context(1.0);
    let mut draw = |ui: &mut egui::Ui| {
        views::contents(ui, &state, &mut Vec::new());
        let _ = export::contents(ui, &state);
    };
    let told = settled(&ctx, [400.0, 600.0], &mut draw);
    let named = |name: &str| told.called(name).unwrap_or_else(|| panic!("{name}"));
    // The saved view and its Delete say whose they are; the field says what it is for.
    assert_eq!(named("Front").role, Role::Button);
    assert_eq!(named("Delete Front").role, Role::Button);
    assert_eq!(
        named("Name (optional), for the view to be saved").role,
        Role::TextInput
    );
    // The buttons of the actions are named by them, after the label they show.
    for id in [
        "scene.view.save",
        "scene.save.picture",
        "scene.save.turntable",
    ] {
        let command = commands::find(id).expect("listed");
        let wanted = commands::spoken(command, command.label);
        assert!(told.called(&wanted).is_some(), "{wanted}");
    }
    let nameless = told
        .nodes
        .iter()
        .filter(|node| node.focusable && node.name.is_empty());
    assert_eq!(nameless.count(), 0);
}

#[test]
fn a_document_card_says_its_kind_and_title_and_whether_it_is_open() {
    let state = with_a_report();
    let (told, window) = told_of(&state, Tab::View);
    let said = cards(&told, &window);
    assert!(
        said.contains(&"Report, Made-up report, selected".to_owned()),
        "{said:?}"
    );
    // The others are not open.
    assert!(
        said.iter().any(|name| name == "Report (PDF), Made-up page"),
        "{said:?}"
    );
}

#[test]
fn the_field_of_the_open_search_is_named_by_what_it_says_before_anything_is_typed() {
    let state = thick_scan_ready();
    let ctx = seen::context(1.0);
    let mut search = Search::default();
    search.open();
    let mut draw = |ui: &mut egui::Ui| {
        let _ = search.show(ui.ctx(), &state);
    };
    let told = settled(&ctx, [900.0, 600.0], &mut draw);
    let fields = told.of(Role::TextInput);
    assert_eq!(fields.len(), 1);
    let field = fields[0];
    assert_eq!(field.placeholder, "Search actions, parts, scans and help");
    assert_eq!(
        field.name, field.placeholder,
        "the words it shows are its name"
    );
}
