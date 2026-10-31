//! Fitting the window to the screen it opens on.

use super::*;

/// A laptop screen of 1920 by 1080 pixels shown at 150%.
const LAPTOP: Area = Area {
    at: [0.0, 0.0],
    size: [1280.0, 720.0],
};
/// What a window's frame adds to the size inside it: the edges and the title bar.
const FRAME: [f32; 2] = [16.0, 39.0];

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01
}

#[test]
fn the_free_area_of_a_screen_leaves_room_for_the_taskbar_along_its_bottom() {
    let free = free_area(LAPTOP);
    assert_eq!(free.at, [0.0, 0.0]);
    assert_eq!(free.size, [1280.0, 672.0]);
    let beside = Area {
        at: [-1440.0, -200.0],
        size: [1440.0, 900.0],
    };
    assert_eq!(
        free_area(beside),
        Area {
            at: [-1440.0, -200.0],
            size: [1440.0, 852.0]
        },
        "a screen beside the first keeps its own corner"
    );
}

#[test]
fn a_remembered_size_that_fits_is_taken_and_the_window_left_where_it_is() {
    let placed = fit(
        Some([1000.0, 600.0]),
        [40.0, 20.0],
        FRAME,
        free_area(LAPTOP),
    );
    assert_eq!(
        placed,
        Placement {
            inner: [1000.0, 600.0],
            at: None
        }
    );
}

#[test]
fn a_remembered_size_that_fits_but_hangs_off_the_screen_is_centred() {
    let placed = fit(
        Some([1000.0, 600.0]),
        [400.0, 300.0],
        FRAME,
        free_area(LAPTOP),
    );
    assert_eq!(placed.inner, [1000.0, 600.0]);
    let at = placed.at.expect("moved back onto the screen");
    assert!(near(at, [132.0, 16.5]), "{at:?}");
}

#[test]
fn a_remembered_size_too_large_for_the_screen_gives_most_of_it_centred() {
    // The 1440 by 900 Voxa used to open at, on a 1080p laptop at 150%.
    let placed = fit(Some([1440.0, 900.0]), [0.0, 0.0], FRAME, free_area(LAPTOP));
    assert!(near(placed.inner, [1136.0, 565.8]), "{placed:?}");
    let at = placed.at.expect("centred");
    assert!(near(at, [64.0, 33.6]), "{at:?}");
}

#[test]
fn with_nothing_remembered_the_window_takes_most_of_the_screen_centred() {
    let large = Area {
        at: [0.0, 0.0],
        size: [3840.0, 2160.0],
    };
    let placed = fit(None, [100.0, 100.0], FRAME, free_area(large));
    assert!(near(placed.inner, [3440.0, 1861.8]), "{placed:?}");
    let at = placed.at.expect("centred");
    assert!(near(at, [192.0, 105.6]), "{at:?}");
}

#[test]
fn a_window_on_a_second_screen_is_placed_on_that_screen() {
    let second = Area {
        at: [-1920.0, 0.0],
        size: [1920.0, 1080.0],
    };
    let placed = fit(None, [-1800.0, 50.0], FRAME, free_area(second));
    let at = placed.at.expect("centred");
    assert!(near(at, [-1824.0, 51.6]), "{at:?}");
}

#[test]
fn on_a_screen_too_small_for_the_smallest_window_its_title_bar_stays_on_the_screen() {
    let tiny = Area {
        at: [10.0, 20.0],
        size: [600.0, 450.0],
    };
    let placed = fit(None, [0.0, 0.0], FRAME, free_area(tiny));
    assert_eq!(
        placed.inner, SMALLEST_WINDOW,
        "no smaller than Voxa can be used at"
    );
    assert_eq!(
        placed.at,
        Some([10.0, 20.0]),
        "its top left corner on the screen"
    );
}

#[test]
fn the_series_list_starts_hidden_in_a_narrow_window() {
    assert!(!list_starts_shown(640.0, 1.0));
    assert!(!list_starts_shown(900.0, 1.0));
    assert!(list_starts_shown(960.0, 1.0), "half a 1920 screen");
    assert!(list_starts_shown(1280.0, 1.0));
    assert!(
        !list_starts_shown(1280.0, 1.5),
        "a larger interface puts fewer points across the same window"
    );
    assert!(list_starts_shown(1440.0, 0.75));
}

#[test]
fn choosing_a_tab_is_remembered() {
    use super::super::panel::Tab;
    use crate::prefs::{self, Prefs};
    use crate::state::AppState;
    let folder = tempfile::tempdir().expect("a temporary folder");
    let mut screen = Screen::new(Arrangement::default(), false, false);
    assert_eq!(screen.panel.tab, Tab::View, "the first tab as Voxa starts");
    // Chosen, and the panel folded, in this run...
    screen.panel.show_tab(Tab::Scene);
    screen.panel.toggle_fold();
    let kept = Prefs::of(&AppState::default(), screen.arrangement(1.0));
    assert_eq!((kept.tab.as_str(), kept.panel_folded), ("Scene", true));
    prefs::save(folder.path(), &kept).expect("saves");
    // ...and open the same way in the next.
    let next = Screen::new(prefs::load(folder.path()).arrangement(), false, false);
    assert_eq!((next.panel.tab, next.panel.folded), (Tab::Scene, true));
}

#[test]
fn a_panel_width_is_kept_only_once_it_has_been_dragged() {
    use crate::prefs::PANEL_SHARE;
    let mut screen = Screen::new(Arrangement::default(), false, false);
    assert_eq!(screen.panel.share, PANEL_SHARE);
    assert_eq!(
        screen.arrangement(1.0).panel_share,
        None,
        "the width Voxa chose is not the person's"
    );
    screen.panel.share = 0.33;
    assert_eq!(screen.arrangement(1.0).panel_share, Some(0.33));
    let remembered = Arrangement {
        panel_share: Some(0.3),
        ..Arrangement::default()
    };
    let screen = Screen::new(remembered, false, false);
    assert_eq!(screen.panel.share, 0.3);
    assert_eq!(screen.arrangement(1.0).panel_share, Some(0.3));
}
