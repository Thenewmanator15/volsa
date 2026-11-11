//! The keys that step through the slices of the active viewport, as the wheel does.

use super::*;
use crate::action::Action;

#[test]
fn the_arrows_and_page_keys_step_the_active_viewport_as_the_wheel_does() {
    use egui::Key;
    let on = |key| shortcut_action(key, 2);
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
    assert_eq!(on(Key::ArrowDown), scroll(1));
    assert_eq!(on(Key::ArrowUp), scroll(-1));
    assert_eq!(on(Key::PageDown), scroll(10));
    assert_eq!(on(Key::PageUp), scroll(-10));
    let first = Action::SetSlice {
        viewport: 2,
        slice: 0,
    };
    assert_eq!(on(Key::Home), Some(first));
    // Going to a slice stops at the last, so the last is asked for as the largest there is.
    let last = Action::SetSlice {
        viewport: 2,
        slice: usize::MAX,
    };
    assert_eq!(on(Key::End), Some(last));
    assert_eq!(on(Key::ArrowLeft), None, "left and right are not Voxa's");
    let (_, pages) = SHORTCUTS
        .iter()
        .find(|(key, _)| key.starts_with("Page Up"))
        .expect("listed");
    assert!(pages.contains("Ten slices"), "{pages}");
}
