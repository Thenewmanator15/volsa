//! Walking the rail with the keyboard: a card is reached as a screen reader or the Tab key
//! reaches it, the up and down arrows move the keyboard from card to card, and Enter opens.

use egui::Key;
use egui::accesskit::NodeId;

use super::window::{PANEL, RAIL, SIZE, Told, Window};
use crate::action::Action;
use crate::state::AppState;
use crate::testing::{id, indexed, series_files, thick_scan_ready};

/// What the card that has the keyboard is called, if a card has it.
fn holder(told: &Told) -> Option<&str> {
    let node = told.focused()?;
    Some(node.name.as_str()).filter(|name| name.contains(" slices"))
}

/// The node of the card called this, which starts with the name.
fn card(told: &Told, name: &str) -> NodeId {
    let found = told.nodes.iter().find(|node| node.name.starts_with(name));
    found.unwrap_or_else(|| panic!("no card {name}")).id
}

#[test]
fn the_rail_is_walked_with_the_arrow_keys() {
    let state = thick_scan_ready();
    let mut window = Window::new();
    let told = window.told(&state);
    let thick = "Axial scan, 30 slices · 5 mm";
    let fine = "Axial scan, 64 slices · 5 mm";
    let scout = "Axial scan, 3 slices · 5 mm";
    // The keyboard is put on the first card as a screen reader puts it on one.
    let told = window.focus(&state, card(&told, thick));
    assert_eq!(
        holder(&told),
        Some("Axial scan, 30 slices · 5 mm, selected")
    );
    // Down goes to the card after, and Up to the card before; the ends hold.
    let mut at = |key| holder(&window.press(&state, key).0).map(ToOwned::to_owned);
    assert_eq!(at(Key::ArrowDown).as_deref(), Some(fine));
    assert_eq!(at(Key::ArrowDown).as_deref(), Some(scout));
    assert_eq!(
        at(Key::ArrowDown).as_deref(),
        Some(scout),
        "no card after the last"
    );
    assert_eq!(at(Key::ArrowUp).as_deref(), Some(fine));
    assert_eq!(
        at(Key::ArrowUp).as_deref(),
        Some("Axial scan, 30 slices · 5 mm, selected")
    );
    assert_eq!(
        at(Key::ArrowUp).as_deref(),
        Some("Axial scan, 30 slices · 5 mm, selected")
    );
}

#[test]
fn enter_opens_the_card_that_has_the_keyboard() {
    let state = thick_scan_ready();
    let mut window = Window::new();
    let told = window.told(&state);
    let told = window.focus(&state, card(&told, "Axial scan, 64 slices"));
    assert!(holder(&told).is_some_and(|name| name.starts_with("Axial scan, 64 slices")));
    let (_, asked) = window.press(&state, Key::Enter);
    let show = Action::SelectSeries {
        viewport: state.active,
        series: id(crate::testing::FINE_SCAN),
    };
    assert!(asked.contains(&show), "{asked:?}");
}

#[test]
fn the_arrows_walk_the_rail_while_a_card_has_the_keyboard_and_step_the_slices_otherwise() {
    let state = thick_scan_ready();
    let mut window = Window::new();
    let told = window.told(&state);
    // With no card in hand the arrow steps through the slices, as it always has.
    let (_, asked) = window.press(&state, Key::ArrowDown);
    let steps = |asked: &[Action]| {
        asked
            .iter()
            .any(|action| matches!(action, Action::Scroll { .. }))
    };
    assert!(steps(&asked), "{asked:?}");
    // With a card in hand it walks the rail, and the slices stay where they are. (A widget
    // with the keyboard already keeps the keys from the actions: the arrows are the rail's.)
    let _ = window.focus(&state, card(&told, "Axial scan, 30 slices"));
    let (told, asked) = window.press(&state, Key::ArrowDown);
    assert!(!steps(&asked), "{asked:?}");
    assert!(holder(&told).is_some_and(|name| name.starts_with("Axial scan, 64 slices")));
}

#[test]
fn the_button_of_a_cards_menu_is_walked_from_too() {
    let state = thick_scan_ready();
    let mut window = Window::new();
    let told = window.told(&state);
    let more = told
        .nodes
        .iter()
        .find(|node| node.name == "More for Axial scan, 30 slices · 5 mm")
        .expect("the card's menu button");
    let told = window.focus(&state, more.id);
    assert_eq!(told.focused().map(|node| node.id), Some(more.id));
    let (told, _) = window.press(&state, Key::ArrowDown);
    assert!(holder(&told).is_some_and(|name| name.starts_with("Axial scan, 64 slices")));
}

/// An export of `count` series of one slice more each than the one before.
fn many(count: i32) -> AppState {
    let files = (0..count)
        .flat_map(|at| series_files(&format!("1.2.7.{at}"), at + 1, at + 1, 4))
        .collect();
    let mut state = AppState::default();
    crate::reducer::reduce(&mut state, Action::IndexProgress(indexed(files)));
    state
}

#[test]
fn a_card_out_of_sight_is_scrolled_to_as_the_arrows_reach_it() {
    let state = many(40);
    let mut window = Window::new();
    let told = window.told(&state);
    // Only some of the forty are drawn, so the last of them are far below.
    assert!(
        told.nodes
            .iter()
            .all(|node| !node.name.starts_with("Axial scan, 40 slices"))
    );
    let _ = window.focus(&state, card(&told, "Axial scan, 1 slice"));
    let mut last = String::new();
    for _ in 0..39 {
        let (told, _) = window.press(&state, Key::ArrowDown);
        last = holder(&told).map(str::to_owned).unwrap_or_default();
    }
    assert!(last.starts_with("Axial scan, 40 slices"), "{last}");
}

/// Which part of the window a node is in, numbered in the order Tab is to go through them: the
/// top bar, the rail, the pictures, the panel and the notices along the bottom.
fn part_of(node: &super::window::Heard) -> u8 {
    let middle = node.at.expect("it is placed").center();
    if middle.y < TOP_BAR {
        0
    } else if middle.y > SIZE[1] - BOTTOM_LINE {
        4
    } else if middle.x < RAIL {
        1
    } else if middle.x > SIZE[0] - PANEL {
        3
    } else {
        2
    }
}

/// How far down the top bar reaches, at the most.
const TOP_BAR: f32 = 40.0;
/// How far up from the bottom of the window the bottom line reaches, at the most.
const BOTTOM_LINE: f32 = 40.0;

#[test]
fn tab_goes_through_the_top_bar_the_rail_the_pictures_and_the_panel_in_that_order() {
    let state = thick_scan_ready();
    let mut window = Window::new();
    let _ = window.told(&state);
    let mut parts = Vec::new();
    for _ in 0..70 {
        let (told, _) = window.press(&state, Key::Tab);
        parts.extend(told.focused().map(part_of));
    }
    // Once through, in order, and then round again from the top bar.
    let wraps = parts.windows(2).position(|pair| pair[1] < pair[0]);
    let once = &parts[..wraps.map_or(parts.len(), |at| at + 1)];
    assert!(once.is_sorted(), "{parts:?}");
    for part in 0..4 {
        assert!(once.contains(&part), "nothing in part {part}: {parts:?}");
    }
    let after = wraps.and_then(|at| parts.get(at + 1));
    assert_eq!(after, Some(&0), "{parts:?}");
}

/// A made-up notice, such as the first screen of an export with other files in it has.
fn a_notice() -> crate::state::Notice {
    crate::state::Notice {
        kind: crate::state::NoticeKind::Info,
        text: "2 files are not scans or documents Volsa lists".to_owned(),
        details: vec!["notes.txt".to_owned()],
        key: None,
    }
}

#[test]
fn tab_reaches_the_notices_last_and_then_goes_round_to_the_top_bar() {
    let mut state = thick_scan_ready();
    state.notices.push(a_notice());
    let mut window = Window::new();
    let _ = window.told(&state);
    let mut parts = Vec::new();
    for _ in 0..90 {
        let (told, _) = window.press(&state, Key::Tab);
        parts.extend(told.focused().map(part_of));
    }
    let wraps = parts.windows(2).position(|pair| pair[1] < pair[0]);
    let once = &parts[..wraps.map_or(parts.len(), |at| at + 1)];
    assert!(once.is_sorted(), "{parts:?}");
    for part in 0..5 {
        assert!(once.contains(&part), "nothing in part {part}: {parts:?}");
    }
    assert_eq!(once.last(), Some(&4), "the notices come last: {parts:?}");
    let after = wraps.and_then(|at| parts.get(at + 1));
    assert_eq!(after, Some(&0), "{parts:?}");
}

/// Press Tab until the keyboard is on a viewport; gives what was told then.
fn tab_to_a_viewport(window: &mut Window, state: &AppState) -> Told {
    for _ in 0..90 {
        let (told, _) = window.press(state, Key::Tab);
        let on_a_viewport = told
            .focused()
            .is_some_and(|node| node.name.contains(" slice "));
        if on_a_viewport {
            return told;
        }
    }
    panic!("Tab never reached a viewport");
}

#[test]
fn with_the_keyboard_on_a_viewport_the_arrows_step_its_slices_and_it_keeps_the_keyboard() {
    let state = thick_scan_ready();
    let mut window = Window::new();
    let _ = window.told(&state);
    let told = tab_to_a_viewport(&mut window, &state);
    let viewport = told.focused().map(|node| node.id);
    let step = |delta| Action::Scroll { viewport: 0, delta };
    // The first press is the one that gives the viewport the lock; the next ones are answered.
    for (key, delta) in [(Key::ArrowDown, 1), (Key::ArrowDown, 1), (Key::ArrowUp, -1)] {
        let (told, asked) = window.press(&state, key);
        assert!(asked.contains(&step(delta)), "{key:?}: {asked:?}");
        assert_eq!(
            told.focused().map(|node| node.id),
            viewport,
            "{key:?} kept it"
        );
    }
    for (key, delta) in [(Key::PageDown, 10), (Key::PageUp, -10)] {
        let (_, asked) = window.press(&state, key);
        assert!(asked.contains(&step(delta)), "{key:?}: {asked:?}");
    }
}

#[test]
fn with_the_keyboard_on_a_button_the_letters_of_the_window_still_work() {
    let state = thick_scan_ready();
    let mut window = Window::new();
    let _ = window.told(&state);
    let (told, _) = window.press(&state, Key::Tab);
    assert!(told.focused().is_some(), "Tab put the keyboard on a button");
    let (_, asked) = window.press(&state, Key::H);
    let flip = Action::Flip {
        viewport: 0,
        horizontal: true,
    };
    assert!(asked.contains(&flip), "{asked:?}");
    // But Space and Enter are the button's own, not the cine's.
    let (_, asked) = window.press(&state, Key::Space);
    assert!(
        !asked
            .iter()
            .any(|action| matches!(action, Action::CineToggle { .. })),
        "{asked:?}"
    );
}

#[test]
fn a_key_a_viewport_lets_go_of_is_not_taken_twice_when_a_tool_consumes_it() {
    // Keyboard measuring (Task 22) claims the arrows while a tool is in hand by consuming them
    // in the pass in which the viewport is drawn: the keys of the window never see them.
    let state = thick_scan_ready();
    let mut window = Window::new();
    let _ = window.told(&state);
    let _ = tab_to_a_viewport(&mut window, &state);
    let ctx = window.ctx.clone();
    let consume = |ctx: &egui::Context| {
        ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::ArrowDown));
    };
    let down = |pressed| egui::Event::Key {
        key: Key::ArrowDown,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let mut taken = Vec::new();
    let _ = super::window::pass(&ctx, SIZE, vec![down(true), down(false)], &mut |ui| {
        consume(ui.ctx());
        taken = crate::ui::keys::pressed_keys(ui.ctx());
    });
    assert_eq!(taken, []);
}
