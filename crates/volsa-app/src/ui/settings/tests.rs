//! The Settings window: asked for from the top bar's menu and by Ctrl+, , and each choice in it
//! doing what the list of actions says.

use egui::{Event, Key, Modifiers, Ui};

use super::*;
use crate::ui::commands::{self, Does, Place};
use crate::ui::hand::Hand;
use crate::ui::keys::Held;

/// What a pass of the window asked for.
#[derive(Default)]
struct Given {
    actions: Vec<Action>,
    asked: Vec<Request>,
}

/// The window, opened, in a hand's window, drawn in `theme`.
fn opened(theme: ThemeChoice) -> (Hand, impl FnMut(&mut Ui) -> Given) {
    let hand = Hand::new(900.0, 700.0);
    open(&hand.ctx);
    let state = AppState::default();
    let draw = move |ui: &mut Ui| {
        let mut given = Given::default();
        show(
            ui.ctx(),
            &state,
            theme,
            &mut given.actions,
            &mut given.asked,
        );
        given
    };
    (hand, draw)
}

/// Click the button a screen reader calls `name` in the window: what the click asked for.
fn click(name: &str, theme: ThemeChoice) -> Given {
    let (hand, mut draw) = opened(theme);
    let mut last = Given::default();
    let mut record = |ui: &mut Ui| {
        let given = draw(ui);
        if !given.actions.is_empty() || !given.asked.is_empty() {
            last = given;
        }
    };
    assert!(hand.click_named(name, &mut record), "{name} is drawn");
    last
}

#[test]
fn ctrl_comma_and_the_menu_ask_for_the_settings() {
    let command = commands::find("settings.open").expect("listed");
    assert_eq!(command.place, Place::Menu);
    assert!(matches!(command.does, Does::Ask(Request::Settings)));
    let held = Held {
        command: true,
        shift: false,
    };
    let state = AppState::default();
    let found = commands::for_key(Key::Comma, held, &state).map(|found| found.id);
    assert_eq!(found, Some("settings.open"));
    let shown = command.key.map(|key| key.shown);
    assert_eq!(shown, Some("Ctrl+,"));
}

#[test]
fn the_window_is_drawn_only_while_open_with_each_heading_and_esc_closes_it() {
    let (hand, mut draw) = opened(ThemeChoice::System);
    let mut drawing = |ui: &mut Ui| {
        let _ = draw(ui);
    };
    let seen = hand.settle(&mut drawing);
    for words in [TITLE, THEME, SIZE, SIDES] {
        assert!(seen.words.iter().any(|word| word == words), "{words}");
    }
    for choice in ["System", "Light", "Dark", "75%", "200%", size::KEYS_SAY] {
        assert!(seen.words.iter().any(|word| word == choice), "{choice}");
    }
    close(&hand.ctx);
    let seen = hand.settle(&mut drawing);
    assert!(!seen.words.iter().any(|word| word == TITLE), "closed");
    // Esc closes it when it is the floating window opened last.
    open(&hand.ctx);
    let _ = hand.settle(&mut drawing);
    let esc = |pressed| Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let mut with_esc = |ui: &mut Ui| {
        let (mut about, mut help, mut tags) = (false, false, false);
        let all = floating::Open {
            about: &mut about,
            help: &mut help,
            tags: &mut tags,
        };
        floating::close_on_esc(ui.ctx(), all);
        drawing(ui);
    };
    let _ = hand.pass(Vec::new(), &mut with_esc);
    let _ = hand.pass(vec![esc(true), esc(false)], &mut with_esc);
    assert!(!is_open(&hand.ctx), "Esc closes it");
}

#[test]
fn choosing_a_theme_asks_for_that_theme() {
    for (name, choice) in [
        ("Use the light theme", ThemeChoice::Light),
        ("Use the dark theme", ThemeChoice::Dark),
        (
            "Follow the system's light or dark setting",
            ThemeChoice::System,
        ),
    ] {
        let given = click(name, ThemeChoice::Dark);
        assert_eq!(given.asked, [Request::Theme(choice)], "{name}");
        assert_eq!(given.actions, [], "{name} changes no state");
    }
}

#[test]
fn choosing_a_size_asks_for_it() {
    for (name, size) in [
        ("Draw the interface at 125%", 1.25),
        ("100%, Draw the interface at its usual size", 1.0),
    ] {
        let given = click(name, ThemeChoice::System);
        assert_eq!(given.asked, [Request::Size(size)], "{name}");
    }
}

#[test]
fn the_patients_left_switch_turns_the_convention_round() {
    let command = commands::find(RADIOLOGICAL).expect("listed");
    let name = &commands::spoken(command, command.label);
    let given = click(name, ThemeChoice::System);
    assert_eq!(given.actions, [Action::ToggleRadiological]);
    assert_eq!(given.asked, []);
}
