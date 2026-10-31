//! The panel at the right of the window: its tabs, its width and its fold.

use egui::Ui;

use super::super::commands::{self, Request};
use super::super::hand::Hand;
use super::*;
use crate::state::AppState;

/// The window the panel is drawn in.
const WIDE: f32 = 1400.0;
const HIGH: f32 = 900.0;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// What the panel gave in the last pass it drew, and where it was.
struct Drew {
    rect: Rect,
    asked: Vec<Request>,
    shown: Vec<Tab>,
}

impl Drew {
    fn new() -> Self {
        Self {
            rect: Rect::NOTHING,
            asked: Vec::new(),
            shown: Vec::new(),
        }
    }
}

/// Draw the panel in a window, as the window does, with the tab it shows named in its
/// contents. `drew` keeps what the last pass gave.
fn draws<'a>(
    panel: &'a mut PanelState,
    state: &'a AppState,
    drew: &'a mut Drew,
) -> impl FnMut(&mut Ui) + 'a {
    move |ui| {
        let contents = |_: &mut Ui, tab: Tab| {
            drew.shown.push(tab);
            vec![Request::About]
        };
        let wide = panel.width(ui.ctx().content_rect().width());
        let shown = show(ui, panel, state, wide, contents);
        drew.rect = shown.rect;
        drew.asked = shown.requests;
    }
}

#[test]
fn the_panel_shows_the_five_tabs_in_order() {
    let titles = ["View", "Measure", "3D", "Labels", "Tools"];
    assert_eq!(Tab::ALL.map(Tab::title), titles);
    let (mut panel, state, mut drew) = (PanelState::default(), AppState::default(), Drew::new());
    let hand = Hand::new(WIDE, HIGH);
    let seen = hand.settle(&mut draws(&mut panel, &state, &mut drew));
    let drawn: Vec<&str> = seen
        .words
        .iter()
        .map(String::as_str)
        .filter(|word| titles.contains(word))
        .collect();
    assert_eq!(drawn, titles);
    assert_eq!(
        drew.shown.last(),
        Some(&Tab::View),
        "and the tab chosen is shown"
    );
}

#[test]
fn the_panel_is_at_the_right_and_holds_what_its_tab_asks() {
    let (mut panel, state, mut drew) = (PanelState::default(), AppState::default(), Drew::new());
    let hand = Hand::new(WIDE, HIGH);
    let _ = hand.settle(&mut draws(&mut panel, &state, &mut drew));
    assert!(near(drew.rect.right(), WIDE), "{:?}", drew.rect);
    assert!(
        near(drew.rect.width(), width(WIDE, panel.share)),
        "{:?}",
        drew.rect
    );
    assert!(near(drew.rect.height(), HIGH), "{:?}", drew.rect);
    assert_eq!(
        drew.asked,
        [Request::About],
        "what the tab asks of the window is passed on"
    );
}

#[test]
fn clicking_a_tab_shows_it() {
    let (mut panel, state, mut drew) = (PanelState::default(), AppState::default(), Drew::new());
    let hand = Hand::new(WIDE, HIGH);
    let clicked = hand.click_named("Show the 3D tab", &mut draws(&mut panel, &state, &mut drew));
    assert!(clicked, "the 3D tab has a button");
    assert_eq!(panel.tab, Tab::Scene);
    let _ = hand.settle(&mut draws(&mut panel, &state, &mut drew));
    assert_eq!(drew.shown.last(), Some(&Tab::Scene));
}

#[test]
fn a_folded_panel_is_a_strip_of_tab_icons_with_names() {
    let mut panel = PanelState {
        folded: true,
        ..PanelState::default()
    };
    let (state, mut drew) = (AppState::default(), Drew::new());
    let hand = Hand::new(WIDE, HIGH);
    let seen = hand.settle(&mut draws(&mut panel, &state, &mut drew));
    assert!(near(drew.rect.width(), STRIP), "{:?}", drew.rect);
    assert!(near(drew.rect.right(), WIDE), "{:?}", drew.rect);
    assert!(drew.shown.is_empty(), "no tab is open to hold anything");
    // Five icons, each named as its action is, in the order of the tabs.
    let names: Vec<&str> = Tab::ALL
        .iter()
        .map(|tab| commands::find(tab.command()).expect("listed").name)
        .collect();
    let tops: Vec<f32> = names
        .iter()
        .map(|name| seen.bounds_of(name).expect("named").top())
        .collect();
    assert!(
        tops.windows(2).all(|pair| pair[0] < pair[1]),
        "in the order of the tabs: {tops:?}"
    );
    assert!(
        seen.named().contains(&"Show or hide the tab panel"),
        "and one that unfolds it"
    );
    // Each icon is a button inside the strip, and none of the tabs' words are drawn.
    for name in &names {
        let bounds = seen.bounds_of(name).expect("named");
        assert!(
            drew.rect.contains_rect(bounds),
            "{name}: {bounds:?} in {:?}",
            drew.rect
        );
        assert!(
            bounds.width() >= 24.0 && bounds.height() >= 24.0,
            "{name}: {bounds:?}"
        );
    }
    assert!(
        !seen.words.iter().any(|word| word == "Measure"),
        "{:?}",
        seen.words
    );
}

#[test]
fn clicking_an_icon_of_a_folded_panel_opens_that_tab() {
    let mut panel = PanelState {
        folded: true,
        ..PanelState::default()
    };
    let (state, mut drew) = (AppState::default(), Drew::new());
    let hand = Hand::new(WIDE, HIGH);
    let name = "Show the Labels tab";
    assert!(hand.click_named(name, &mut draws(&mut panel, &state, &mut drew)));
    assert!(!panel.folded);
    assert_eq!(panel.tab, Tab::Labels);
    let _ = hand.settle(&mut draws(&mut panel, &state, &mut drew));
    assert!(drew.rect.width() > STRIP, "{:?}", drew.rect);
}

#[test]
fn the_fold_button_folds_the_panel_and_unfolds_it() {
    let mut panel = PanelState::default();
    let (state, mut drew) = (AppState::default(), Drew::new());
    let hand = Hand::new(WIDE, HIGH);
    let fold = "Show or hide the tab panel";
    assert!(hand.click_named(fold, &mut draws(&mut panel, &state, &mut drew)));
    assert!(panel.folded);
    let _ = hand.settle(&mut draws(&mut panel, &state, &mut drew));
    assert!(near(drew.rect.width(), STRIP), "{:?}", drew.rect);
    assert!(hand.click_named(fold, &mut draws(&mut panel, &state, &mut drew)));
    assert!(!panel.folded);
    assert_eq!(panel.tab, Tab::View, "the tab that was open is open again");
}

#[test]
fn the_panel_is_held_between_260_points_and_two_fifths() {
    // Its share of the window, when that is between the limits.
    assert!(near(width(1400.0, 0.25), 350.0));
    // No narrower than 260 points, so that the tabs' names fit.
    assert!(near(width(1400.0, 0.05), 260.0));
    assert!(near(width(800.0, 0.1), 260.0));
    // No wider than two fifths of the window, so that the pictures keep the rest.
    assert!(near(width(1400.0, 0.9), 560.0));
    assert!(near(width(800.0, 0.9), 320.0));
    // In a window too narrow for 260 points to be two fifths of it, two fifths holds.
    assert!(near(width(640.0, 0.1), 256.0));
    assert!(near(width(640.0, 0.9), 256.0));
    assert!(near(width(400.0, 0.3), 160.0));
}

#[test]
fn dragging_the_edge_changes_the_share_as_far_as_the_limits_allow() {
    // The edge pulled to the left makes the panel wider.
    let wider = dragged(1400.0, 0.25, -70.0);
    assert!(near(wider * 1400.0, 420.0), "{wider}");
    let narrower = dragged(1400.0, 0.25, 40.0);
    assert!(near(narrower * 1400.0, 310.0), "{narrower}");
    assert!(near(dragged(1400.0, 0.25, -5000.0) * 1400.0, 560.0));
    assert!(near(dragged(1400.0, 0.25, 5000.0) * 1400.0, 260.0));
    // What is kept is the share, so that a larger window gives the panel as much more.
    assert!(
        near(width(1800.0, wider), 540.0),
        "{}",
        width(1800.0, wider)
    );
}

#[test]
fn a_tab_is_kept_by_its_own_name_and_one_not_known_opens_on_view() {
    for tab in Tab::ALL {
        assert_eq!(Tab::from_name(tab.name()), Some(tab));
    }
    // Its name does not change with the words it is shown under.
    assert_eq!(Tab::Scene.name(), "Scene");
    assert_eq!(Tab::Scene.title(), "3D");
    let opened = |name: &str| PanelState::remembered(None, false, name).tab;
    assert_eq!(opened("Labels"), Tab::Labels);
    assert_eq!(opened("3D"), Tab::View, "a title is not a name");
    assert_eq!(opened("Elsewhere"), Tab::View);
    assert_eq!(opened(""), Tab::View);
}

#[test]
fn a_remembered_width_and_fold_are_taken_and_a_nonsense_width_is_not() {
    let kept = PanelState::remembered(Some(0.3), true, "Tools");
    assert_eq!((kept.share, kept.folded, kept.tab), (0.3, true, Tab::Tools));
    let start = PanelState::remembered(None, false, "View");
    assert_eq!(start.share, crate::prefs::PANEL_SHARE);
    assert_eq!(PanelState::default().share, crate::prefs::PANEL_SHARE);
}

#[test]
fn showing_a_tab_unfolds_the_panel() {
    let mut panel = PanelState::remembered(None, true, "View");
    panel.show_tab(Tab::Measure);
    assert_eq!((panel.tab, panel.folded), (Tab::Measure, false));
    panel.toggle_fold();
    assert!(panel.folded);
    assert_eq!(panel.tab, Tab::Measure, "folding keeps the tab");
}

#[test]
fn every_tab_has_an_action_that_shows_it() {
    let state = AppState::default();
    for tab in Tab::ALL {
        let command = commands::find(tab.command()).expect("listed");
        assert_eq!(command.label, tab.title());
        let does = command.does.clone().run(&state, &mut Vec::new());
        assert_eq!(does, Some(Request::ShowTab(tab)));
    }
    let fold = commands::find("panel.fold").expect("listed");
    assert_eq!(
        fold.does.clone().run(&state, &mut Vec::new()),
        Some(Request::FoldPanel)
    );
}

#[test]
fn the_icons_of_a_folded_panel_fit_in_a_window_as_short_as_the_smallest_at_twice_the_size() {
    // 640 by 480 with the interface at twice its size is 320 by 240 points, of which the bars
    // above and below take some: the strip has about 180 to hold its fold button and five icons.
    let mut panel = PanelState {
        folded: true,
        ..PanelState::default()
    };
    let (state, mut drew) = (AppState::default(), Drew::new());
    let short = 180.0;
    let hand = Hand::new(WIDE, short);
    let seen = hand.settle(&mut draws(&mut panel, &state, &mut drew));
    let mut names: Vec<&str> = Tab::ALL
        .iter()
        .map(|tab| commands::find(tab.command()).expect("listed").name)
        .collect();
    names.push("Show or hide the tab panel");
    for name in names {
        let bounds = seen.bounds_of(name).expect("named");
        assert!(
            bounds.bottom() <= short && bounds.height() >= 20.0,
            "{name}: {bounds:?} runs past the bottom of {short}"
        );
    }
}
