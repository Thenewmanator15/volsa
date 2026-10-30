//! What a viewport says of a volume of a series that holds several, and the keys that step
//! between them; and what it says of a picture in colour.

use egui::Key;
use voxa_core::{SeriesId, WindowLevel};

use super::tests::{series, study};
use super::*;
use crate::action::Action;

#[test]
fn a_volume_of_several_says_which_it_is_under_the_series() {
    let wl = WindowLevel::new(0.0, 1.0);
    let mut second = series();
    second.set = Some(voxa_core::VolumeSet {
        of: SeriesId("1.2.3".to_owned()),
        at: 1,
        count: 2,
        what: "echo 83 ms".to_owned(),
    });
    let corners = corner_text(&study(), &second, 0, Some(1.0), wl, false);
    // Below the series' name, where it has the room, not beside the scale at the bottom.
    assert_eq!(corners[1][1], "Volume 2 of 2 · echo 83 ms");
    assert_eq!(corners[2][0], "Slice 1 of 30");
    let plain = corner_text(&study(), &series(), 0, Some(1.0), wl, false);
    assert_eq!(plain[1].len(), 1, "a series of one says nothing of volumes");
}

#[test]
fn a_picture_in_colour_says_nothing_of_a_window() {
    let wl = WindowLevel::new(0.0, 1.0);
    let mut colour = series();
    colour.colour = true;
    let corners = corner_text(&study(), &colour, 0, Some(1.0), wl, false);
    assert_eq!(
        corners[3],
        Vec::<String>::new(),
        "it is shown in its own colours"
    );
    let grey = corner_text(&study(), &series(), 0, Some(1.0), wl, false);
    assert_eq!(grey[3], ["W 1  L 0"]);
}

#[test]
fn the_square_brackets_step_between_volumes() {
    use super::super::commands::{Does, for_key};
    let state = crate::state::AppState {
        active: 1,
        ..crate::state::AppState::default()
    };
    let on = |key| match for_key(key, super::super::keys::Held::default(), &state)?.does {
        Does::Act(act) => act(&state),
        Does::Ask(_) => None,
    };
    let step = |by| Some(Action::StepVolume { viewport: 1, by });
    assert_eq!(on(Key::OpenBracket), step(-1));
    assert_eq!(on(Key::CloseBracket), step(1));
}
