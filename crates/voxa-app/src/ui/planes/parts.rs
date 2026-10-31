//! Labelled parts in the plane views: which part lies under a point, what a click on it
//! asks for, and what the pointer is told when it rests on one.

use voxa_core::{Patient, Point};
use voxa_labels::LabelSet;
use voxa_render::Overlay;

use super::mouse::PlaneInput;
use crate::action::Action;
use crate::annotations::Tool;
use crate::labels::{name_of, on_this_series};
use crate::state::AppState;

/// What a panel that draws labels says of them, in its note.
pub const LABELS_NOTE: &str = "Labels: made elsewhere";
/// What it says of labels that Voxa made.
pub const LABELS_NOTE_BY_VOXA: &str = "Labels: made by Voxa";

/// What it says of labels that a plugin the person added made.
pub const LABELS_NOTE_ADDED: &str = "Labels: added by you, not part of Voxa";

/// What a panel that draws labels says of them: that another program made them, that Voxa
/// did, or that a plugin the person added did.
pub fn labels_note(state: &AppState) -> &'static str {
    match &state.labels.made {
        Some(made) if made.added.is_some() => LABELS_NOTE_ADDED,
        Some(_) => LABELS_NOTE_BY_VOXA,
        None => LABELS_NOTE,
    }
}

/// Whether the plane views draw labels now: there are labels on this series, and they are
/// not switched off over the planes.
pub fn drawn(state: &AppState) -> bool {
    on_this_series(state) && state.labels.overlay != Overlay::Off
}

/// The part at a point of the patient, if there is one there and it is shown. A hidden part
/// is not there.
pub fn part_under(state: &AppState, set: &LabelSet, at: Point<Patient>) -> Option<usize> {
    set.part_at(at).filter(|part| state.labels.is_shown(*part))
}

/// What a press of the main button in a plane view asks of the labels: with the navigating
/// tool, and labels drawn there, to select the part pressed on, or none where there is none.
/// The same press moves the position: one click means one point.
pub fn selected_by(state: &AppState, set: Option<&LabelSet>, input: &PlaneInput) -> Option<Action> {
    let set = set.filter(|_| drawn(state) && state.tool == Tool::Navigate)?;
    let pressed = input.pressed_at?;
    Some(Action::PartSelect(part_under(state, set, pressed)))
}

/// What the pointer is told when it rests on a part in a plane view: the part's name, with
/// its side if it has one.
pub fn named_under(state: &AppState, set: Option<&LabelSet>, input: &PlaneInput) -> Option<String> {
    let set = set.filter(|_| drawn(state))?;
    let part = part_under(state, set, input.hover_at?)?;
    state.labels.parts.get(part).map(name_of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reducer::reduce;
    use crate::testing::labelled_head as labelled;

    fn on_the_right() -> Point<Patient> {
        Point::new(0.5, 1.0, 70.0)
    }
    fn on_the_left() -> Point<Patient> {
        Point::new(2.5, 1.0, 70.0)
    }
    fn off_the_labels() -> Point<Patient> {
        Point::new(40.0, 1.0, 70.0)
    }

    fn pressed(at: Point<Patient>) -> PlaneInput {
        PlaneInput {
            pressed_at: Some(at),
            held_at: Some(at),
            hover_at: Some(at),
            ..PlaneInput::default()
        }
    }

    #[test]
    fn labels_that_voxa_made_are_not_said_to_be_made_elsewhere() {
        let (mut state, _) = labelled();
        assert_eq!(labels_note(&state), "Labels: made elsewhere");
        state.labels.made = Some(voxa_labels::Made::default());
        assert_eq!(labels_note(&state), "Labels: made by Voxa");
    }

    #[test]
    fn the_part_under_a_point_is_one_that_is_shown() {
        let (mut state, set) = labelled();
        assert_eq!(part_under(&state, &set, on_the_right()), Some(0));
        assert_eq!(part_under(&state, &set, on_the_left()), Some(1));
        assert_eq!(part_under(&state, &set, off_the_labels()), None);
        reduce(
            &mut state,
            Action::PartShow {
                part: 1,
                shown: false,
            },
        );
        assert_eq!(
            part_under(&state, &set, on_the_left()),
            None,
            "a hidden part is not there"
        );
    }

    #[test]
    fn a_press_selects_the_part_there_or_none_and_only_with_the_navigating_tool() {
        let (mut state, set) = labelled();
        let on = |state: &AppState, at| selected_by(state, Some(&set), &pressed(at));
        assert_eq!(on(&state, on_the_left()), Some(Action::PartSelect(Some(1))));
        assert_eq!(
            on(&state, off_the_labels()),
            Some(Action::PartSelect(None)),
            "on nothing"
        );
        let moved = PlaneInput {
            hover_at: Some(on_the_left()),
            ..PlaneInput::default()
        };
        assert_eq!(
            selected_by(&state, Some(&set), &moved),
            None,
            "moving selects nothing"
        );
        assert_eq!(
            selected_by(&state, None, &pressed(on_the_left())),
            None,
            "no labels kept"
        );
        reduce(&mut state, Action::SetTool(Tool::Ruler));
        assert_eq!(
            on(&state, on_the_left()),
            None,
            "a ruler is being drawn, not a part chosen"
        );
        reduce(&mut state, Action::SetTool(Tool::Navigate));
        reduce(&mut state, Action::SetOverlay(Overlay::Off));
        assert_eq!(
            on(&state, on_the_left()),
            None,
            "what is not drawn is not picked"
        );
    }

    #[test]
    fn the_pointer_is_told_the_name_and_side_of_the_part_it_rests_on() {
        let (mut state, set) = labelled();
        let over = |state: &AppState, at| {
            let input = PlaneInput {
                hover_at: Some(at),
                ..PlaneInput::default()
            };
            named_under(state, Some(&set), &input)
        };
        assert_eq!(over(&state, on_the_left()).as_deref(), Some("half, left"));
        assert_eq!(over(&state, on_the_right()).as_deref(), Some("half, right"));
        assert_eq!(over(&state, off_the_labels()), None);
        reduce(
            &mut state,
            Action::PartShow {
                part: 1,
                shown: false,
            },
        );
        assert_eq!(
            over(&state, on_the_left()),
            None,
            "a hidden part has no name to give"
        );
        state.labels.study = Some(voxa_core::StudyId("another".to_owned()));
        assert_eq!(
            over(&state, on_the_right()),
            None,
            "labels of another study are not here"
        );
        assert!(!drawn(&state));
    }
}
