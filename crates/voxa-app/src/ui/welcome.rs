//! The middle of the window before anything is open: how to begin, in words at the size of the
//! rest of the text, with the two ways to open something beneath them. The rail says the same
//! quietly, but it is folded in a narrow window, and the middle is where a person looks.

use egui::{Align, Layout, Rect, Ui, UiBuilder, vec2};

use super::commands::{self, Request};
use super::theme;
use crate::action::Action;
use crate::state::AppState;

/// What the middle says before anything is open.
pub const BEGIN: &str = "Open a scan export to begin: a folder, a zip file or a DICOMDIR. You can \
                         also drop it on this window.";
/// The most the text is wide, in points; it wraps beyond that.
const WIDEST: f32 = 460.0;

/// Whether the window has nothing open, so that the middle says how to begin: no export is open
/// and no document stands in its place.
pub fn is_empty(state: &AppState) -> bool {
    state.index.is_none() && state.document.is_none()
}

/// Draw how to begin in `area`, with the buttons that open a folder, and a zip file or a
/// DICOMDIR, from the list of actions. Gives what the buttons asked of the window.
pub fn show(ui: &mut Ui, area: Rect, state: &AppState, actions: &mut Vec<Action>) -> Vec<Request> {
    let mut asked = Vec::new();
    ui.painter()
        .rect_filled(area, theme::ROUNDING, theme::palette(ui.ctx()).panel);
    let wide = (area.width() - 32.0).clamp(0.0, WIDEST);
    let room = Rect::from_center_size(area.center(), vec2(wide, area.height()));
    let layout = Layout::top_down(Align::Center);
    ui.scope_builder(UiBuilder::new().max_rect(room).layout(layout), |ui| {
        // About a third of the way down, where the eye comes to rest.
        ui.add_space((area.height() * 0.3).max(0.0));
        ui.label(BEGIN);
        ui.add_space(8.0);
        for id in ["open.folder", "open.file"] {
            commands::offer(ui, id, state, None, actions, &mut asked);
        }
    });
    asked
}

#[cfg(test)]
mod tests {
    use egui::Rect;

    use super::*;
    use crate::ui::hand::Hand;
    use crate::ui::top::Pick;

    /// What the middle draws in a space of this size, and what its buttons asked in a click on
    /// the one named `clicked`, if one is.
    fn drawn(
        size: [f32; 2],
        state: &AppState,
        clicked: Option<&str>,
    ) -> (crate::ui::hand::Seen, Vec<Request>) {
        let hand = Hand::new(size[0], size[1]);
        let mut asked = Vec::new();
        let mut draw = |ui: &mut Ui| {
            let whole = ui.available_rect_before_wrap();
            let asks = show(ui, whole, state, &mut Vec::new());
            asked = asks;
        };
        let seen = hand.settle(&mut draw);
        if let Some(name) = clicked {
            assert!(
                hand.click_named(name, &mut draw),
                "{name} is there to click"
            );
        }
        (seen, asked)
    }

    #[test]
    fn the_middle_of_an_empty_window_says_how_to_begin_and_offers_the_ways_to_open() {
        let (seen, _) = drawn([700.0, 500.0], &AppState::default(), None);
        assert!(
            seen.words.iter().any(|word| word == BEGIN),
            "{:?}",
            seen.words
        );
        for id in ["open.folder", "open.file"] {
            let command = commands::find(id).expect("listed");
            let name = commands::spoken(command, command.label);
            assert!(seen.bounds_of(&name).is_some(), "{name}: {:?}", seen.names);
        }
        // The words are in the middle of the space, not tucked in a corner, at the size of the
        // rest of the words, and the buttons are beneath them.
        let words = seen.text_bounds(BEGIN).expect("drawn");
        assert!(words.top() > 50.0 && words.top() < 250.0, "{words:?}");
        let folder = commands::find("open.folder").expect("listed");
        let button = seen.bounds_of(&commands::spoken(folder, folder.label));
        assert!(
            button.is_some_and(|button| button.top() >= words.bottom()),
            "{button:?}"
        );
        let body = seen
            .placed
            .iter()
            .map(Rect::height)
            .fold(f32::MAX, f32::min);
        assert!(words.height() >= body, "{words:?}");
    }

    #[test]
    fn the_buttons_of_the_middle_ask_to_open_a_folder_or_a_file() {
        let state = AppState::default();
        for (id, pick) in [("open.folder", Pick::Folder), ("open.file", Pick::File)] {
            let command = commands::find(id).expect("listed");
            let name = commands::spoken(command, command.label);
            let (_, asked) = drawn([700.0, 500.0], &state, Some(&name));
            assert_eq!(asked, [Request::Open(pick)], "{id}");
        }
    }

    #[test]
    fn it_fits_a_small_space_and_goes_when_something_is_open() {
        let (seen, _) = drawn([240.0, 200.0], &AppState::default(), None);
        let space = Rect::from_min_size(egui::Pos2::ZERO, vec2(240.0, 200.0));
        for (name, at) in &seen.names {
            assert!(space.contains_rect(*at), "{name}: {at:?}");
        }
        assert!(is_empty(&AppState::default()));
        assert!(!is_empty(&crate::testing::thick_scan_ready()));
        let document = AppState {
            document: Some(crate::state::OpenDocument {
                entry: "DOCS/D0001".to_owned(),
                report: None,
            }),
            ..AppState::default()
        };
        assert!(!is_empty(&document));
    }
}
