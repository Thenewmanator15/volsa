//! The keys that step through the slices of the active viewport, as the wheel does.

use super::super::commands::{Does, find};
use super::*;
use crate::action::Action;
use crate::state::AppState;

#[test]
fn the_arrows_and_page_keys_step_the_active_viewport_as_the_wheel_does() {
    let state = AppState {
        active: 2,
        ..AppState::default()
    };
    let on = |id| match find(id).expect("listed").does {
        Does::Act(act) => act(&state),
        Does::Ask(_) => None,
    };
    let scroll = |delta| Some(Action::Scroll { viewport: 2, delta });
    // The wheel turned towards the person moves on to later slices, and so does Down.
    let wheel = ViewportInput {
        wheel: 1,
        ..ViewportInput::default()
    };
    assert_eq!(
        input_to_actions(2, &wheel),
        [Action::Scroll {
            viewport: 2,
            delta: 1
        }]
    );
    assert_eq!(on("slice.after"), scroll(1));
    assert_eq!(on("slice.before"), scroll(-1));
    assert_eq!(on("slice.ten_on"), scroll(10));
    assert_eq!(on("slice.ten_back"), scroll(-10));
    let first = Action::SetSlice {
        viewport: 2,
        slice: 0,
    };
    assert_eq!(on("slice.first"), Some(first));
    // Going to a slice stops at the last, so the last is asked for as the largest there is.
    let last = Action::SetSlice {
        viewport: 2,
        slice: usize::MAX,
    };
    assert_eq!(on("slice.last"), Some(last));
    let pages = find("slice.ten_back").expect("listed");
    assert!(pages.name.contains("Ten slices"), "{}", pages.name);
    for (id, key) in [
        ("slice.after", egui::Key::ArrowDown),
        ("slice.ten_back", egui::Key::PageUp),
    ] {
        let stepping = find(id).and_then(|command| command.key);
        assert!(stepping.is_some_and(|shortcut| shortcut.key == key && steps_slices(key)));
    }
}
