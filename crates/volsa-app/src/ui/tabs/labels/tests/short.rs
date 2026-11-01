//! The Labels tab in a short window: the part that stays in place scrolls too, and the list keeps
//! its room.

use super::*;

#[test]
fn in_a_short_window_the_list_keeps_room_and_the_fixed_part_scrolls_with_the_check_first() {
    let (mut state, check) = made_and_failed();
    // The eyeball is chosen, which adds its controls to the part that stays in place.
    reduce(&mut state, Action::PartSelect(Some(3)));
    // The room the tab has in a 640 by 480 window: a panel of 240 points inside, and about 350
    // down from the row of tabs.
    let (wide, high) = (240.0, 350.0);
    let (hand, mut view) = (Hand::new(wide, high), PartsView::default());
    let mut draw = |ui: &mut Ui| {
        let _ = show(ui, &state, &mut view, &mut Vec::new());
    };
    let seen = hand.settle(&mut draw);
    // The model's own check is still the first thing in the tab, in sight.
    let check_at = bounds_of(&seen, &check);
    assert!(check_at.top() < 0.1 * high, "{check_at:?}");
    // And what scrolls beneath the part that stays has at least a third of the tab.
    let list = bounds_of(&seen, "Groups");
    assert!(
        list.top() <= 0.67 * high,
        "the list begins at {} of {high}",
        list.top()
    );
    let overview = bounds_of(&seen, "Go to Deep brain");
    assert!(
        overview.bottom() <= high,
        "a row of the list is in sight: {overview:?}"
    );
    // The part that stays does not all fit, and can be scrolled to its end, where the part
    // chosen is.
    let controls = ["Search parts", "Show only this"];
    let in_sight = |seen: &Seen, name: &str| {
        let place = seen.bounds_of(name);
        place.is_some_and(|place| place.bottom() <= list.top())
    };
    let mut reached = controls.map(|name| in_sight(&seen, name));
    assert!(
        !reached.iter().all(|found| *found),
        "it all fits: {reached:?}"
    );
    let at = check_at.center();
    for _ in 0..8 {
        hand.scroll_at(at, -120.0, &mut draw);
        let now = hand.settle(&mut draw);
        for (name, found) in controls.iter().zip(reached.iter_mut()) {
            *found |= in_sight(&now, name);
        }
    }
    assert!(reached.iter().all(|found| *found), "{reached:?}");
}

#[test]
fn in_a_tall_window_nothing_about_the_fixed_part_scrolls() {
    let (mut state, _) = made_and_failed();
    reduce(&mut state, Action::PartSelect(Some(3)));
    let (hand, mut view) = (Hand::new(WIDE, 900.0), PartsView::default());
    let mut draw = |ui: &mut Ui| {
        let _ = show(ui, &state, &mut view, &mut Vec::new());
    };
    let seen = hand.settle(&mut draw);
    // Everything that stays in place is in sight above the list, as it always was.
    let fixed = bounds_of(&seen, "Show only this");
    let list = bounds_of(&seen, "Groups");
    assert!(fixed.bottom() <= list.top(), "{fixed:?} then {list:?}");
}
