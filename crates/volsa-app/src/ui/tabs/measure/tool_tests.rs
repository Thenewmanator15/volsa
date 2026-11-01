//! The Measure tab says on the screen how to draw with the tool in hand.

use super::tests::{Kept, NARROW, measured_twice};
use super::*;
use crate::ui::hand::Hand;

#[test]
fn a_tool_in_hand_says_on_the_screen_how_to_draw_with_it_and_how_to_put_it_down() {
    let drawn = |tool| {
        let state = AppState {
            tool,
            ..measured_twice()
        };
        let (hand, mut kept) = (Hand::new(NARROW, 900.0), Kept::default());
        hand.settle(&mut |ui| kept.draw(ui, &state)).words
    };
    // Each tool says what its own button's hint says, then how to put it down.
    for (tool, id) in TOOLS {
        let said = drawn(tool);
        let explains = commands::find(id).expect("listed").explains;
        let how = said.iter().find(|word| word.ends_with("Esc puts it down."));
        if tool == Tool::Navigate {
            assert!(how.is_none(), "nothing in hand to put down: {said:?}");
        } else {
            let how = how.unwrap_or_else(|| panic!("{tool:?} says nothing of Esc: {said:?}"));
            assert!(how.starts_with(explains), "{how:?} against {explains:?}");
        }
    }
}
