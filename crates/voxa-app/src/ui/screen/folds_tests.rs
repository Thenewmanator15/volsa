//! Folding the side columns in a narrow window, so that the pictures keep their room.

use egui::{Rect, Ui};

use super::super::super::hand::Hand;
use super::super::super::panel::{self, Tab};
use super::*;
use crate::prefs::{self, Arrangement};
use crate::state::AppState;

/// Where the pictures are in a window `wide` by `high` points, with the side columns laid
/// out as the window lays them out: the rail at the left, or its strip while it is folded, the
/// panel at the right, and the pictures in what is left, within the middle's margin.
fn pictures_in(screen: &mut Screen, wide: f32, high: f32) -> Rect {
    let state = AppState::default();
    let mut room = Rect::NOTHING;
    let mut draw = |ui: &mut Ui| {
        let widths = screen.widths(ui.ctx().content_rect().width());
        let rail = egui::Panel::left("rail").exact_size(widths.rail);
        rail.resizable(false).show(ui, |_| {});
        let nothing = |_: &mut Ui, _: Tab| Vec::new();
        let _ = panel::show(ui, &mut screen.panel, &state, widths.panel, nothing);
        let middle = egui::CentralPanel::default().frame(super::super::super::middle());
        middle.show(ui, |ui| room = ui.available_rect_before_wrap());
    };
    let _ = Hand::new(wide, high).settle(&mut draw);
    room
}

/// The window as a 0.6.0 settings file with the series list and the column dragged as wide
/// as they went leaves it.
fn widest_of_0_6_0() -> Arrangement {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let text = r#"{"window": [640.0, 480.0], "list_share": 0.35, "column_share": 0.6}"#;
    std::fs::write(folder.path().join(prefs::FILE), text).expect("writes");
    prefs::load(folder.path()).arrangement()
}

#[test]
fn the_pictures_keep_200_points_in_the_smallest_window() {
    let mut screen = Screen::new(widest_of_0_6_0(), false, false);
    // Both open in a wider window, and the window made as small as it goes.
    screen.list_shown = true;
    screen.settle(1200.0);
    screen.settle(640.0);
    assert!(!screen.list_shown, "the rail folds");
    assert!(screen.panel.folded, "and the panel");
    let room = pictures_in(&mut screen, 640.0, 480.0);
    assert!(room.width() >= PICTURES_LEAST, "{room:?}");
    // Either opened again keeps the pictures their room, and folds the other.
    screen.show_tab(Tab::Measure);
    screen.settle(640.0);
    assert!(!screen.panel.folded && !screen.list_shown);
    let room = pictures_in(&mut screen, 640.0, 480.0);
    assert!(room.width() >= PICTURES_LEAST, "{room:?}");
    screen.toggle_rail();
    screen.settle(640.0);
    assert!(
        screen.list_shown && screen.panel.folded,
        "opening the rail folds the panel"
    );
    let room = pictures_in(&mut screen, 640.0, 480.0);
    assert!(room.width() >= PICTURES_LEAST, "{room:?}");
}

#[test]
fn an_open_column_shrinks_to_leave_the_pictures_their_room() {
    // At twice the interface's usual size the smallest window is 320 points across.
    let mut screen = Screen::new(widest_of_0_6_0(), false, false);
    screen.settle(320.0);
    screen.show_tab(Tab::View);
    screen.settle(320.0);
    assert!(!screen.panel.folded);
    let room = pictures_in(&mut screen, 320.0, 240.0);
    assert!(room.width() >= PICTURES_LEAST - 0.01, "{room:?}");
    // Just wider than a narrow window, both columns may be open, and both give way.
    let mut screen = Screen::new(widest_of_0_6_0(), false, false);
    screen.list_shown = true;
    screen.settle(770.0);
    assert!(screen.list_shown && !screen.panel.folded);
    let room = pictures_in(&mut screen, 770.0, 600.0);
    assert!(room.width() >= PICTURES_LEAST - 0.01, "{room:?}");
}

#[test]
fn folds_the_window_made_itself_are_not_kept_as_the_persons() {
    let mut screen = Screen::new(Arrangement::default(), false, false);
    screen.settle(700.0);
    assert!(screen.panel.folded);
    assert!(
        !screen.arrangement(1.0).panel_folded,
        "the person left it open"
    );
    screen.toggle_panel();
    screen.settle(700.0);
    assert!(!screen.panel.folded);
    screen.toggle_panel();
    screen.settle(700.0);
    assert!(
        screen.arrangement(1.0).panel_folded,
        "folded by the person, and kept so"
    );
}

#[test]
fn in_a_wide_window_the_columns_open_and_fold_only_when_asked() {
    let mut screen = Screen::new(Arrangement::default(), false, false);
    screen.list_shown = true;
    screen.settle(1200.0);
    assert!(screen.list_shown && !screen.panel.folded);
    screen.settle(700.0);
    assert!(
        !screen.list_shown && screen.panel.folded,
        "narrow: both fold"
    );
    screen.settle(1200.0);
    assert!(
        !screen.list_shown && screen.panel.folded,
        "wide again, they are not opened for the person"
    );
    screen.toggle_rail();
    screen.show_tab(Tab::Labels);
    screen.settle(1200.0);
    assert!(
        screen.list_shown && !screen.panel.folded,
        "both open as asked"
    );
}

#[test]
fn nothing_is_folded_while_the_window_is_still_being_sized_as_it_opens() {
    let mut screen = Screen::new(Arrangement::default(), false, false);
    screen.awaits(1200.0);
    screen.settle(512.0);
    assert!(!screen.panel.folded, "not yet the window's own size");
    screen.settle(1200.0);
    screen.settle(512.0);
    assert!(screen.panel.folded, "once it was, a narrow window folds");
    // A window that never takes the size asked is folded for in the end.
    let mut screen = Screen::new(Arrangement::default(), false, false);
    screen.awaits(1200.0);
    for _ in 0..20 {
        screen.settle(512.0);
    }
    assert!(screen.panel.folded);
}

#[test]
fn a_tab_asked_for_as_the_window_opens_stays_open_in_a_narrow_window() {
    // As a picture of the window asks for a tab before its first frame.
    let mut screen = Screen::new(Arrangement::default(), false, false);
    screen.show_tab(Tab::Measure);
    screen.list_shown = true;
    screen.rail_shown_as_it_opens();
    screen.settle(640.0);
    assert!(!screen.panel.folded, "the tab asked for is shown");
    assert!(!screen.list_shown, "and the series list folds in its place");
}
