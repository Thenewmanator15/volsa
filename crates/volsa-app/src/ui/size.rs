//! The size the interface is drawn at: the choice of sizes in the Settings window, beside the
//! keys that egui itself answers to (Ctrl+= and Ctrl+- for larger and smaller, Ctrl+0 for the
//! usual size).

use egui::{Label, RichText, Ui};

use super::commands::{self, Does, Request};
use super::theme;
use crate::state::AppState;

/// The sizes the Settings window offers: 1 is the interface's usual size.
pub const SIZES: [f32; 8] = [0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

/// What is said under the sizes: the keys that change the size from anywhere.
pub const KEYS_SAY: &str = "Ctrl+= larger, Ctrl+- smaller, Ctrl+0 usual";

/// An interface size as a percentage of the usual size, to the nearest whole one: `125%`.
pub fn percent(size: f32) -> String {
    format!("{:.0}%", size * 100.0)
}

/// The action that draws the interface at `size`, from the list of actions.
fn action_for(size: f32) -> Option<&'static commands::Command> {
    commands::all().iter().find(|command| {
        matches!(command.does, Does::Ask(Request::Size(at)) if percent(at) == percent(size))
    })
}

/// The sizes to choose between, each an action of the list of actions, with the size now
/// marked, and under them the keys. Gives the size chosen, if one was.
pub fn choices(ui: &mut Ui, state: &AppState) -> Option<f32> {
    let now = ui.ctx().zoom_factor();
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        for command in SIZES.into_iter().filter_map(action_for) {
            let this = command.label == percent(now);
            let pressed = commands::button(ui, command.id, state, Some(this));
            if let Some(Does::Ask(Request::Size(size))) = pressed {
                chosen = Some(size);
            }
        }
    });
    let quiet = theme::palette(ui.ctx()).quiet_text;
    ui.add(Label::new(RichText::new(KEYS_SAY).small().color(quiet)).wrap());
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::INTERFACE_SIZES;
    use crate::ui::hand::Hand;

    #[test]
    fn the_interface_size_is_said_in_percent() {
        assert_eq!(percent(1.0), "100%");
        assert_eq!(percent(1.25), "125%");
        assert_eq!(percent(0.9), "90%");
        assert_eq!(percent(1.300_000_1), "130%");
    }

    #[test]
    fn the_sizes_offered_run_from_smaller_to_larger_and_each_can_be_remembered() {
        assert!(SIZES.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(SIZES.contains(&1.0), "the usual size");
        assert!(SIZES.iter().all(|size| INTERFACE_SIZES.contains(size)));
        assert!(KEYS_SAY.contains("Ctrl+0"));
    }

    #[test]
    fn each_size_is_offered_and_choosing_one_gives_it() {
        let hand = Hand::new(400.0, 300.0);
        let state = AppState::default();
        let mut chosen = Vec::new();
        let mut draw = |ui: &mut Ui| chosen.extend(choices(ui, &state));
        let seen = hand.settle(&mut draw);
        for size in SIZES {
            let label = percent(size);
            assert!(seen.words.contains(&label), "{label}: {:?}", seen.words);
        }
        assert!(seen.words.iter().any(|word| word == KEYS_SAY));
        assert!(hand.click_named("Draw the interface at 75%", &mut draw));
        assert!(hand.click_named("Draw the interface at 150%", &mut draw));
        assert_eq!(chosen, [0.75, 1.5]);
    }
}
