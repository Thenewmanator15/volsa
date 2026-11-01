//! The list of actions: every entry once, no key twice, the keys doing what they did, and
//! the help listing them.

use std::collections::HashSet;

use egui::{Event, Key, Modifiers, RawInput};

use super::super::help;
use super::super::keys::{self, Held};
use super::*;
use crate::annotations::Tool;
use crate::state::Layout;
use crate::testing::thick_scan_ready;

/// What an action gives when it is run: a change of state, or something asked of the window.
#[derive(Debug, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "a few made and compared in a test"
)]
enum Gives {
    Act(Action),
    Ask(Request),
}

/// What `does` gives in `state`, if anything.
fn gives(does: Does, state: &AppState) -> Option<Gives> {
    match does {
        Does::Act(act) => act(state).map(Gives::Act),
        Does::Ask(request) => Some(Gives::Ask(request)),
    }
}

const NONE: Held = Held {
    command: false,
    shift: false,
};
const CTRL: Held = Held {
    command: true,
    shift: false,
};
const SHIFT: Held = Held {
    command: false,
    shift: true,
};
const CTRL_SHIFT: Held = Held {
    command: true,
    shift: true,
};

/// What pressing `key` with `held` gives in `state`, whether or not it can be used now.
fn on(key: Key, held: Held, state: &AppState) -> Option<Gives> {
    let command = for_key(key, held, state)?;
    gives(command.does.clone(), state)
}

#[test]
fn ids_are_unique() {
    let ids: HashSet<&str> = all().iter().map(|command| command.id).collect();
    assert_eq!(ids.len(), all().len());
}

#[test]
fn no_two_commands_share_a_key() {
    let keyed = all()
        .iter()
        .filter_map(|command| command.key)
        .map(|shortcut| (shortcut.key, shortcut.command, shortcut.shift));
    let others = OTHER_KEYS
        .iter()
        .map(|(key, held, _)| (*key, held.command, held.shift));
    let pressed: Vec<_> = keyed.chain(others).collect();
    let different: HashSet<_> = pressed.iter().collect();
    assert_eq!(different.len(), pressed.len(), "{pressed:?}");
    for (_, _, id) in OTHER_KEYS {
        assert!(
            find(id).is_some_and(|command| command.key.is_some()),
            "{id}"
        );
    }
}

/// What 0.6.0's keys gave with the first viewport active and no tool in hand, written out
/// from its own list of keys, its `shortcut_action` and `keys::pressed`, and their tests. Esc,
/// which gave the 3D view the large part of the window, is not among them: there is no large
/// part now.
fn what_0_6_0_gave() -> Vec<(Key, Held, Gives)> {
    let viewport = 0;
    let act = Gives::Act;
    let ask = Gives::Ask;
    let flip = |horizontal| {
        act(Action::Flip {
            viewport,
            horizontal,
        })
    };
    let scroll = |delta| act(Action::Scroll { viewport, delta });
    let slice = |slice| act(Action::SetSlice { viewport, slice });
    let step = |by| act(Action::StepVolume { viewport, by });
    let layout = |layout| act(Action::SetLayout(layout));
    vec![
        (Key::H, NONE, flip(true)),
        (Key::V, NONE, flip(false)),
        (Key::R, NONE, act(Action::Rotate { viewport })),
        (Key::F, NONE, act(Action::Fit { viewport })),
        (Key::Num1, NONE, layout(Layout::One)),
        (Key::Num2, NONE, layout(Layout::TwoAcross)),
        (Key::Num4, NONE, layout(Layout::TwoByTwo)),
        (Key::Space, NONE, act(Action::CineToggle { viewport })),
        (Key::ArrowUp, NONE, scroll(-1)),
        (Key::ArrowDown, NONE, scroll(1)),
        (Key::PageUp, NONE, scroll(-10)),
        (Key::PageDown, NONE, scroll(10)),
        (Key::Home, NONE, slice(0)),
        (Key::End, NONE, slice(usize::MAX)),
        (Key::OpenBracket, NONE, step(-1)),
        (Key::CloseBracket, NONE, step(1)),
        (Key::M, NONE, ask(Request::ShowTab(Tab::Measure))),
        (Key::Z, CTRL, act(Action::Undo)),
        (Key::Y, CTRL, act(Action::Redo)),
        (Key::Z, CTRL_SHIFT, act(Action::Redo)),
        (Key::B, NONE, ask(Request::FoldRail)),
        (Key::F11, NONE, ask(Request::FullScreen)),
        (Key::Questionmark, NONE, ask(Request::Help(None))),
        (Key::Questionmark, SHIFT, ask(Request::Help(None))),
    ]
}

#[test]
fn every_key_of_0_6_0_still_does_what_it_did() {
    let state = thick_scan_ready();
    for (key, held, gave) in what_0_6_0_gave() {
        assert_eq!(on(key, held, &state), Some(gave), "{key:?} with {held:?}");
    }
    // With a tool in hand, Esc puts it down before it does anything else.
    for tool in [Tool::Ruler, Tool::Angle, Tool::Region, Tool::Marker] {
        let holding = AppState {
            tool,
            ..thick_scan_ready()
        };
        let down = Gives::Act(Action::SetTool(Tool::Navigate));
        assert_eq!(on(Key::Escape, NONE, &holding), Some(down), "{tool:?}");
    }
    for nothing in [Key::Num3, Key::Z, Key::Y, Key::K, Key::ArrowLeft] {
        assert_eq!(
            on(nothing, NONE, &state),
            None,
            "{nothing:?} is no key of Voxa's"
        );
    }
}

#[test]
fn escape_with_no_tool_in_hand_leaves_the_pictures_as_they_are() {
    // 0.6.0 gave the 3D view the large part of the window; there is no large part now.
    let state = thick_scan_ready();
    assert_eq!(on(Key::Escape, NONE, &state), None);
    assert!(keys::pressed(Key::Escape, NONE, &state).is_none());
    let escape = find("view.escape").expect("listed");
    assert!(!escape.name.contains("3D"), "{}", escape.name);
}

#[test]
fn ctrl_is_held_only_where_a_key_says_and_shift_may_be_held_with_any() {
    let state = thick_scan_ready();
    for letter in [Key::M, Key::B, Key::H, Key::F] {
        assert_eq!(
            on(letter, CTRL, &state),
            None,
            "Ctrl+{letter:?} is not {letter:?}"
        );
    }
    let play = Gives::Act(Action::CineToggle { viewport: 0 });
    assert_eq!(on(Key::Space, SHIFT, &state), Some(play));
    let measure = Gives::Ask(Request::ShowTab(Tab::Measure));
    assert_eq!(on(Key::M, SHIFT, &state), Some(measure));
}

#[test]
fn the_keys_that_size_the_interface_are_listed_and_left_to_egui() {
    let state = AppState::default();
    for (id, key, size) in [
        ("size.larger", Key::Equals, Request::Larger),
        ("size.smaller", Key::Minus, Request::Smaller),
        ("size.100", Key::Num0, Request::Size(1.0)),
    ] {
        let command = find(id).expect("listed");
        let shortcut = command.key.expect("with its key");
        assert_eq!((shortcut.key, shortcut.command), (key, true), "{id}");
        assert!(
            matches!(&command.does, Does::Ask(asked) if *asked == size),
            "{id}"
        );
        assert!(for_key(key, CTRL, &state).is_none(), "egui answers {id}");
        assert!(
            for_key(key, CTRL_SHIFT, &state).is_none(),
            "{id}, with Shift"
        );
    }
    assert!(for_key(Key::Plus, CTRL, &state).is_none());
}

#[test]
fn the_keys_act_on_the_active_viewport() {
    let state = AppState {
        active: 2,
        ..thick_scan_ready()
    };
    let fit = Gives::Act(Action::Fit { viewport: 2 });
    assert_eq!(on(Key::F, NONE, &state), Some(fit));
    let after = Gives::Act(Action::Scroll {
        viewport: 2,
        delta: 1,
    });
    assert_eq!(on(Key::ArrowDown, NONE, &state), Some(after));
}

#[test]
fn a_key_does_nothing_while_its_action_cannot_be_used() {
    let mut state = thick_scan_ready();
    let undo =
        |state: &AppState| keys::pressed(Key::Z, CTRL, state).and_then(|does| gives(does, state));
    assert_eq!(undo(&state), None, "nothing to take back");
    let marker = crate::annotations::Shape::Marker { at: [0.0; 3] };
    let _ = crate::reducer::reduce(&mut state, Action::AnnotationAdd(marker));
    assert_eq!(undo(&state), Some(Gives::Act(Action::Undo)));
}

#[test]
fn the_help_lists_every_command_with_a_key() {
    let with_keys: Vec<_> = all()
        .iter()
        .filter_map(|command| Some((command.key?.shown, command.name)))
        .collect();
    assert_eq!(help::keys_listed(), with_keys);
    // And it draws each, with what it does beside it.
    let words = help::drawn_words(2560.0, 1600.0);
    for (key, name) in with_keys {
        assert!(words.iter().any(|word| word == key), "{key} is not drawn");
        assert!(words.iter().any(|word| word == name), "{name} is not drawn");
    }
}

#[test]
fn every_command_has_a_name_a_label_and_one_other_word() {
    for command in all() {
        assert!(!command.name.is_empty(), "{}", command.id);
        assert!(!command.label.is_empty(), "{}", command.id);
        assert!(!command.words.is_empty(), "{}", command.id);
    }
}

/// A key pressed with nothing held.
fn down(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// What the window's key path takes from a frame in which H, 1, M and / are pressed, after
/// a frame in which a text field was drawn, and given the keyboard if `typing`.
fn typed(typing: bool) -> (Vec<Action>, Vec<Request>) {
    let state = thick_scan_ready();
    let ctx = egui::Context::default();
    let mut text = String::new();
    let field = egui::Id::new("a field");
    let mut taken = (Vec::new(), Vec::new());
    let pressed = [Key::H, Key::Num1, Key::M, Key::Slash].map(down);
    for events in [Vec::new(), pressed.to_vec()] {
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let drawn = ui.add(egui::TextEdit::singleline(&mut text).id(field));
            if typing {
                drawn.request_focus();
            }
            taken = keys::answered(ui.ctx(), &state);
        });
        // No renderer is behind this pass to take the font texture.
        output.textures_delta.clear();
    }
    taken
}

#[test]
fn keys_do_nothing_while_a_text_field_has_focus() {
    let (actions, requests) = typed(true);
    assert_eq!((actions, requests), (Vec::new(), Vec::new()));
    let (actions, requests) = typed(false);
    let flip = Action::Flip {
        viewport: 0,
        horizontal: true,
    };
    assert_eq!(actions, [flip, Action::SetLayout(Layout::One)]);
    assert_eq!(
        requests,
        [Request::ShowTab(Tab::Measure), Request::Search],
        "with the field let go"
    );
}

#[test]
fn a_hint_is_the_name_and_key_then_what_explains_it() {
    let hint_of = |id| hint(find(id).expect("listed"));
    assert_eq!(hint_of("layout.one"), "One viewport (1)");
    assert_eq!(hint_of("open.folder"), "Open a folder of DICOM files");
    assert_eq!(
        hint_of("tab.measure"),
        "Show the Measure tab (M)\n\
         Rulers, angles, regions and markers, and the list of what has been measured"
    );
}

#[test]
fn an_action_for_some_profiles_is_offered_only_for_them() {
    let fit = find("view.fit").expect("listed");
    let for_ct = Command {
        profiles: &[Profile::Ct],
        ..fit.clone()
    };
    let for_mr = Command {
        profiles: &[Profile::Mr],
        ..fit.clone()
    };
    // The made-up scans are MR.
    let shown = thick_scan_ready();
    assert!(offered(fit, &shown) && offered(fit, &AppState::default()));
    assert!(offered(&for_mr, &shown));
    assert!(!offered(&for_ct, &shown));
    assert!(!offered(&for_mr, &AppState::default()), "no scan is shown");
}

#[test]
fn ctrl_k_and_a_slash_open_the_search_and_nothing_else_does() {
    let state = AppState::default();
    let search = Gives::Ask(Request::Search);
    assert_eq!(on(Key::K, CTRL, &state), Some(search));
    assert_eq!(
        on(Key::Slash, NONE, &state),
        Some(Gives::Ask(Request::Search))
    );
    assert_eq!(
        on(Key::K, NONE, &state),
        None,
        "K alone is not a key of Voxa's"
    );
    // On a keyboard where a slash needs Shift it is still a slash: it searches, it does not show
    // the keys. A question mark, which egui reports as a key of its own, shows them.
    assert_eq!(
        on(Key::Slash, SHIFT, &state),
        Some(Gives::Ask(Request::Search))
    );
    assert_eq!(
        on(Key::Questionmark, NONE, &state),
        Some(Gives::Ask(Request::Help(None)))
    );
    let command = find("search.open").expect("listed");
    assert_eq!(command.key.map(|key| key.shown), Some("Ctrl+K or /"));
}

#[test]
fn the_panel_is_folded_by_a_key_as_the_series_list_is_and_each_tab_is_shown_by_its_action() {
    let state = thick_scan_ready();
    // `P` for the panel, as `B` is for the series list that sits at the other side.
    assert_eq!(
        on(Key::P, NONE, &state),
        Some(Gives::Ask(Request::FoldPanel))
    );
    assert_eq!(on(Key::P, CTRL, &state), None);
    // M shows the Measure tab, as it always has; no other tab has a key of its own.
    for tab in Tab::ALL {
        let command = find(tab.command()).expect("listed");
        let key = command.key.map(|key| key.shown);
        let wanted = (tab == Tab::Measure).then_some("M");
        assert_eq!(key, wanted, "{}", command.id);
        assert_eq!(command.place, Place::Panel);
    }
    let measure = on(Key::M, NONE, &state);
    assert_eq!(measure, Some(Gives::Ask(Request::ShowTab(Tab::Measure))));
}
