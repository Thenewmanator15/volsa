//! Labelled parts in the plane views: which part lies under a point, what a click on it
//! asks for, and what the pointer is told when it rests on one.

use voxa_core::{Patient, Point};
use voxa_labels::{LabelSet, Side};
use voxa_render::Overlay;

use super::mouse::PlaneInput;
use crate::action::Action;
use crate::annotations::Tool;
use crate::labels::on_this_series;
use crate::state::AppState;

/// What a panel that draws labels says of them, in its note.
pub const LABELS_NOTE: &str = "Labels: made elsewhere";

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
    let part = state
        .labels
        .parts
        .get(part_under(state, set, input.hover_at?)?)?;
    let side = match part.side {
        Side::Left => ", left",
        Side::Right => ", right",
        Side::Midline => "",
    };
    Some(format!("{}{side}", part.name))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use voxa_core::Affine;
    use voxa_labels::{Part, Trust};

    use super::*;
    use crate::labels::NamesFrom;
    use crate::reducer::reduce;
    use crate::testing::thick_scan_ready;

    /// Two label voxels side by side that between them cover the test series: the first,
    /// part 0, towards the patient's right; the second, part 1, towards their left.
    fn halves() -> LabelSet {
        let part = |side| Part {
            name: "half".to_owned(),
            group: "Halves".to_owned(),
            side,
            colour: [200, 100, 50],
            trust: Trust::Model,
            voxels: 1,
            ml: 1.0,
        };
        LabelSet {
            dims: [2, 1, 1],
            numbers: vec![1, 2],
            placement: Affine::from_columns(
                [1.8, 0.0, 0.0],
                [0.0, 3.6, 0.0],
                [0.0, 0.0, 150.0],
                [0.45, 1.35, 72.5],
            ),
            parts: vec![part(Side::Right), part(Side::Left)],
            title: String::new(),
            verdict: None,
        }
    }

    /// The test series with its volume ready and those labels read on it.
    fn labelled() -> (AppState, Arc<LabelSet>) {
        let set = Arc::new(halves());
        let mut state = thick_scan_ready();
        let open = Action::OpenLabels {
            labels: PathBuf::from("labels.nii"),
            table: None,
        };
        reduce(&mut state, open);
        let read = Action::LabelsRead {
            title: String::new(),
            parts: set.parts.clone(),
            verdict: None,
            corners: set.corners(),
            names_from: NamesFrom::Nothing,
        };
        reduce(&mut state, read);
        assert!(on_this_series(&state), "the labels meet the scan");
        (state, set)
    }

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
