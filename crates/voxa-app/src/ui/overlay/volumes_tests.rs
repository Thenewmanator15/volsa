//! What a viewport says of a volume of a series that holds several, and the keys that step
//! between them.

use egui::Key;
use voxa_core::{SeriesId, WindowLevel};

use super::tests::{series, study};
use super::*;
use crate::action::Action;

#[test]
fn a_volume_of_several_says_which_it_is_above_the_slice() {
    let wl = WindowLevel::new(0.0, 1.0);
    let mut second = series();
    second.set = Some(voxa_core::VolumeSet {
        of: SeriesId("1.2.3".to_owned()),
        at: 1,
        count: 2,
        what: "echo 83 ms".to_owned(),
    });
    let corners = corner_text(&study(), &second, 0, Some(1.0), wl, false);
    assert_eq!(corners[2][0], "Volume 2 of 2 · echo 83 ms");
    assert_eq!(corners[2][1], "Slice 1 of 30");
    let plain = corner_text(&study(), &series(), 0, Some(1.0), wl, false);
    assert_eq!(
        plain[2][0], "Slice 1 of 30",
        "a series of one says nothing of volumes"
    );
}

#[test]
fn the_square_brackets_step_between_volumes() {
    let step = |by| Some(Action::StepVolume { viewport: 1, by });
    assert_eq!(shortcut_action(Key::OpenBracket, 1), step(-1));
    assert_eq!(shortcut_action(Key::CloseBracket, 1), step(1));
}
