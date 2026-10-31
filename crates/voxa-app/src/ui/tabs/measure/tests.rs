//! The Measure tab: clicking down it finds everything it offers, its buttons are the list's
//! own, and the M key shows it in the panel, over nothing.

use egui::{Event, Key, Modifiers, Order, Ui, pos2};
use voxa_core::Point;
use voxa_volume::RegionStats;

use super::*;
use crate::annotations::Shape;
use crate::measuring::NO_SCALE;
use crate::planes::grid;
use crate::reducer::reduce;
use crate::testing::thick_scan_ready;
use crate::ui::commands;
use crate::ui::hand::{Hand, Seen};
use crate::ui::keys;
use crate::ui::panel::{self, PanelState, Tab};

/// A series ready to measure on, with a ruler and a marker measured on it.
fn measured_twice() -> AppState {
    let mut state = thick_scan_ready();
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
    state
}

/// The tab as it is kept between passes, with what it gave in them.
#[derive(Default)]
struct Kept {
    tab: MeasureTab,
    /// What every tab keeps, for the panel drawn whole.
    all: super::super::Kept,
    actions: Vec<Action>,
    requests: Vec<Request>,
    /// How far down and across the contents reached in the last pass.
    bottom: f32,
    right: f32,
}

impl Kept {
    /// Draw the tab for `state` in `ui`, and keep what it gave.
    fn draw(&mut self, ui: &mut Ui, state: &AppState) {
        let asked = show(ui, state, &mut self.tab, &mut self.actions);
        self.requests.extend(asked);
        self.bottom = ui.min_rect().bottom();
        self.right = ui.min_rect().right();
    }

    /// One pass with these events.
    fn pass(&mut self, hand: &Hand, state: &AppState, events: Vec<Event>) -> Seen {
        hand.pass(events, &mut |ui| self.draw(ui, state))
    }
}

/// How wide the contents of a tab are in the narrowest panel: 260 points, less its margins
/// and the bar beside the tab.
const NARROW: f32 = 236.0;

/// Each thing once, in the order first met.
fn once<T: PartialEq>(all: Vec<T>) -> Vec<T> {
    let mut found: Vec<T> = Vec::new();
    for one in all {
        if !found.contains(&one) {
            found.push(one);
        }
    }
    found
}

/// Click on every button of the tab, top to bottom, and gather everything that was asked
/// for, each thing once: the changes of state, and what was asked of the window.
fn sweep(state: &AppState) -> (Vec<Action>, Vec<Request>) {
    let (hand, mut kept) = (Hand::new(NARROW, 900.0), Kept::default());
    hand.click_each(&mut |ui| kept.draw(ui, state));
    (once(kept.actions), once(kept.requests))
}

fn position(x: f64, y: f64, z: f64) -> Action {
    Action::SetPosition(Point::new(x, y, z))
}

#[test]
fn clicking_down_the_tab_finds_everything_it_offers() {
    let state = measured_twice();
    let ids: Vec<u64> = state.annotations.items.iter().map(|item| item.id).collect();
    let (actions, requests) = sweep(&state);
    for tool in [Tool::Ruler, Tool::Angle, Tool::Region, Tool::Marker] {
        assert!(actions.contains(&Action::SetTool(tool)), "{tool:?}");
    }
    let in_hand = Action::SetTool(Tool::Navigate);
    assert!(
        !actions.contains(&in_hand),
        "the tool in hand is not asked for again"
    );
    assert!(actions.contains(&Action::Undo));
    assert!(
        !actions.contains(&Action::Redo),
        "nothing has been taken back"
    );
    for id in ids {
        assert!(actions.contains(&Action::AnnotationRemove(id)), "{id}");
    }
    assert!(
        actions.contains(&position(0.0, 0.0, 10.0)),
        "the ruler's first end"
    );
    assert!(actions.contains(&position(-12.0, 4.5, 30.25)), "the marker");
    assert!(actions.contains(&Action::AnnotationsClear));
    let saves = [Save::Csv, Save::Json].map(Request::SaveMeasurements);
    assert_eq!(requests, saves, "both ways to save are passed on");
    let renamed = |action: &Action| matches!(action, Action::AnnotationRename { .. });
    assert!(
        !actions.iter().any(renamed),
        "a click in a name changes no name"
    );
}

#[test]
fn what_was_taken_back_can_be_done_again() {
    let mut state = measured_twice();
    let _ = reduce(&mut state, Action::Undo);
    let (actions, _) = sweep(&state);
    assert!(
        actions.contains(&Action::Undo),
        "the ruler can be taken back too"
    );
    assert!(actions.contains(&Action::Redo));
}

#[test]
fn with_nothing_measured_there_is_nothing_to_save_clear_or_take_back() {
    let state = thick_scan_ready();
    let (actions, requests) = sweep(&state);
    let tools = [Tool::Ruler, Tool::Angle, Tool::Region, Tool::Marker].map(Action::SetTool);
    assert_eq!(actions, tools, "the tools, and nothing else");
    assert_eq!(requests, []);
    let (hand, mut kept) = (Hand::new(NARROW, 900.0), Kept::default());
    let seen = hand.settle(&mut |ui| kept.draw(ui, &state));
    assert!(
        seen.words.iter().any(|word| word == NOTHING_MEASURED),
        "{:?}",
        seen.words
    );
    assert!(!seen.words.iter().any(|word| word == NO_SCALE));
}

#[test]
fn with_nothing_to_measure_on_the_tab_says_why_and_offers_no_tool() {
    let state = AppState::default();
    let (actions, requests) = sweep(&state);
    assert_eq!(actions, []);
    assert_eq!(requests, []);
    let (hand, mut kept) = (Hand::new(NARROW, 900.0), Kept::default());
    let seen = hand.settle(&mut |ui| kept.draw(ui, &state));
    assert!(
        seen.words.iter().any(|word| word == NO_SCALE),
        "{:?}",
        seen.words
    );
    assert_eq!(
        NO_SCALE,
        "Measuring needs a scan with a scale in its files."
    );
}

#[test]
fn each_row_says_what_was_measured_and_how_exactly() {
    let state = measured_twice();
    let (hand, mut kept) = (Hand::new(NARROW, 900.0), Kept::default());
    let seen = hand.settle(&mut |ui| kept.draw(ui, &state));
    let placement = grid(&state).map(|(placement, _)| placement);
    for item in &state.annotations.items {
        let said = measured(&item.shape, placement.as_ref(), item.scale);
        let has = |text: &str| seen.words.iter().any(|word| word.contains(text));
        assert!(has(&said.value), "{} in {:?}", said.value, seen.words);
        assert!(has(&said.detail), "{}", said.detail);
    }
    assert!(seen.words.iter().any(|word| word == "5.0 mm"));
}

#[test]
fn a_name_is_taken_when_the_person_leaves_the_field() {
    let state = measured_twice();
    let first = state.annotations.items[0].clone();
    let (hand, mut kept) = (Hand::new(NARROW, 900.0), Kept::default());
    let _ = hand.settle(&mut |ui| kept.draw(ui, &state));
    // Click down the left of the tab until a name takes the keys.
    let mut y = 3.0;
    while kept.tab.editing.is_none() && y < kept.bottom {
        hand.click(pos2(30.0, y), &mut |ui| kept.draw(ui, &state));
        y += 4.0;
    }
    let (id, typed) = kept.tab.editing.clone().expect("a name took the keys");
    assert_eq!((id, typed), (first.id, first.label.clone()));
    kept.actions.clear();
    let typing = vec![Event::Text("abc".to_owned())];
    let _ = kept.pass(&hand, &state, typing);
    assert_eq!(kept.actions, [], "typing alone changes no name");
    let (_, typed) = kept.tab.editing.clone().expect("still being typed");
    // Clicks one after another count as a double-click, which selects what is typed over.
    assert!(typed.contains("abc") && typed != first.label, "{typed}");
    // A click on nothing, well below the tab's contents, leaves the field.
    let nowhere = pos2(NARROW / 2.0, 880.0);
    hand.click(nowhere, &mut |ui| kept.draw(ui, &state));
    let label = typed;
    assert_eq!(kept.actions, [Action::AnnotationRename { id, label }]);
    assert_eq!(kept.tab.editing, None);
}

#[test]
fn the_tab_draws_its_buttons_from_the_list() {
    let state = measured_twice();
    let (hand, mut kept) = (Hand::new(NARROW, 900.0), Kept::default());
    let seen = hand.settle(&mut |ui| kept.draw(ui, &state));
    let ids = [
        "measure.move",
        "measure.ruler",
        "measure.angle",
        "measure.region",
        "measure.marker",
        "measure.undo",
        "measure.redo",
        "measure.save_csv",
        "measure.save_json",
        "measure.clear",
    ];
    for id in ids {
        let command = commands::find(id).expect("listed");
        let name = seen.bounds_of(&commands::spoken(command, command.label));
        assert!(name.is_some(), "{id} is named {:?}", seen.named());
        assert!(
            seen.words.iter().any(|word| word == command.label),
            "{id} says {} in {:?}",
            command.label,
            seen.words
        );
    }
}

#[test]
fn the_tab_stays_within_the_narrowest_panel() {
    // A marker's long place and a region's words, in the narrowest the panel is.
    let mut state = measured_twice();
    let inside = RegionStats {
        mean: -1234.5,
        sd: 1234.5,
        min: -2000.0,
        max: 3000.0,
        area_mm2: 1234.5,
        samples: 987_654,
    };
    let region = Shape::Region {
        centre: [1.0, 2.0, 10.0],
        across: [1.0, 0.0, 0.0],
        down: [0.0, 1.0, 0.0],
        radii_mm: [4.0, 3.0],
        stats: Some(inside),
    };
    let _ = reduce(&mut state, Action::AnnotationAdd(region));
    let (hand, mut kept) = (Hand::new(NARROW, 900.0), Kept::default());
    let _ = hand.settle(&mut |ui| kept.draw(ui, &state));
    assert!(kept.right <= NARROW + 0.01, "{}", kept.right);
}

/// What a frame of the window made of its keys and its panel.
#[derive(Default)]
struct Frame {
    /// What the keys pressed asked of the window.
    requests: Vec<Request>,
    /// Every layer shown over the window's background: a window would be one.
    layers: Vec<egui::LayerId>,
}

/// One frame as the window makes it, as far as the panel goes: the keys pressed are read,
/// and the panel is drawn with the tab it shows. Gives what the keys asked of the window.
fn frame(
    hand: &Hand,
    panel: &mut PanelState,
    kept: &mut Kept,
    state: &AppState,
    keys: Vec<Event>,
) -> (Frame, Seen) {
    let mut drew = Frame::default();
    let seen = hand.pass(keys, &mut |ui| {
        drew.requests = keys::answered(ui.ctx(), state).1;
        let contents = |ui: &mut Ui, tab: Tab| {
            let mut actions = Vec::new();
            super::super::show(ui, tab, state, &mut kept.all, &mut actions)
        };
        let wide = panel.width(ui.ctx().content_rect().width());
        let _ = panel::show(ui, panel, state, wide, contents);
        drew.layers = ui
            .ctx()
            .memory(|memory| memory.areas().visible_layer_ids())
            .into_iter()
            .collect();
    });
    (drew, seen)
}

/// The M key, pressed.
fn m_pressed() -> Vec<Event> {
    vec![Event::Key {
        key: Key::M,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]
}

/// Press M in a window whose panel is `panel`, and do what it asks of the panel, as the
/// window does. Gives what the M frame asked, with the layers and words of the frames that
/// came to rest after it.
fn after_m(state: &AppState, panel: &mut PanelState) -> (Frame, Seen) {
    let (hand, mut kept) = (Hand::new(1000.0, 700.0), Kept::default());
    let _ = frame(&hand, panel, &mut kept, state, Vec::new());
    let (pressed, _) = frame(&hand, panel, &mut kept, state, m_pressed());
    for request in &pressed.requests {
        if let Request::ShowTab(tab) = request {
            panel.show_tab(*tab);
        }
    }
    let _ = frame(&hand, panel, &mut kept, state, Vec::new());
    let (after, seen) = frame(&hand, panel, &mut kept, state, Vec::new());
    (
        Frame {
            requests: pressed.requests,
            ..after
        },
        seen,
    )
}

#[test]
fn m_opens_the_measure_tab_and_unfolds_the_panel() {
    let state = measured_twice();
    let mut panel = PanelState::remembered(None, true, "View");
    let (pressed, seen) = after_m(&state, &mut panel);
    assert_eq!(pressed.requests, [Request::ShowTab(Tab::Measure)]);
    assert_eq!((panel.tab, panel.folded), (Tab::Measure, false));
    for word in ["Ruler", "Save as CSV…", "Clear all"] {
        assert!(seen.words.iter().any(|said| said == word), "{word}");
    }
    // Pressed again, it shows the tab that is shown: it does not close anything.
    let (_, seen) = after_m(&state, &mut panel);
    assert_eq!((panel.tab, panel.folded), (Tab::Measure, false));
    assert!(seen.words.iter().any(|said| said == "Ruler"));
}

#[test]
fn no_window_opens_over_the_rail() {
    let state = measured_twice();
    let mut panel = PanelState::remembered(None, true, "View");
    let (after, _) = after_m(&state, &mut panel);
    let panels = |layer: &&egui::LayerId| layer.order == Order::Background;
    assert!(
        after.layers.iter().any(|layer| panels(&layer)),
        "layers are seen"
    );
    let windows = |layer: &&egui::LayerId| layer.order == Order::Middle;
    assert_eq!(
        after.layers.iter().filter(windows).count(),
        0,
        "no window is open: {:?}",
        after.layers
    );
    let titled = egui::Id::new("Measurements");
    assert!(after.layers.iter().all(|layer| layer.id != titled));
}
