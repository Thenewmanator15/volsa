//! The Parts look in the scene: which labelled part is under a point of the picture, and
//! what a click on it asks for.

use voxa_core::{Patient, Point};
use voxa_labels::LabelSet;
use voxa_render::{Look, pick_part};

use super::scene_params;
use crate::action::Action;
use crate::annotations::Tool;
use crate::labels::look_drawn;
use crate::state::AppState;

/// What the scene says of itself in the Parts look.
pub const CAPTION: &str =
    "Solids are labels made elsewhere, drawn as given: Voxa did not find them in this scan.";

/// Whether the scene is drawn as labelled solids now.
pub fn drawn(state: &AppState) -> bool {
    look_drawn(state) == Look::Parts
}

/// The part shown at a point of the scene's picture, given in points from its top left
/// corner, and the point of the patient there: the first shown part on that line of sight,
/// in what the cuts leave. `None` in any other look, and where the picture shows no part.
pub fn part_seen(
    state: &AppState,
    set: &LabelSet,
    viewport: [f32; 2],
    at: [f32; 2],
) -> Option<(usize, Point<Patient>)> {
    if !drawn(state) {
        return None;
    }
    let shown: Vec<bool> = state.labels.shown.iter().map(|how| how.shown).collect();
    pick_part(set, &shown, &scene_params(state, 1.0)?, viewport, at)
}

/// What a click in the scene asks of the labels: in the Parts look, with the navigating
/// tool, to select the part clicked, or none where there is none. The same click moves the
/// position: one click means one point.
pub fn selected_by(state: &AppState, part: Option<usize>) -> Option<Action> {
    (drawn(state) && state.tool == Tool::Navigate).then_some(Action::PartSelect(part))
}

#[cfg(test)]
mod tests {
    use voxa_render::Look;

    use super::*;
    use crate::reducer::reduce;
    use crate::testing::labelled_head;

    /// The labelled test series in the Parts look, seen from the front as the scene starts.
    fn solids() -> (AppState, std::sync::Arc<LabelSet>, [f32; 2]) {
        let (mut state, set) = labelled_head();
        reduce(&mut state, Action::SceneLook(Look::Parts));
        assert!(drawn(&state));
        let viewport = state.scene.viewport;
        (state, set, viewport)
    }

    /// A point of the picture this many millimetres to the right of its middle.
    fn right_of_the_middle(state: &AppState, viewport: [f32; 2], mm: f64) -> [f32; 2] {
        let [wide, high] = viewport;
        #[expect(clippy::cast_possible_truncation, reason = "a few pixels")]
        let across = (mm / state.scene.camera.mm_per_px) as f32;
        [wide / 2.0 + across, high / 2.0]
    }

    #[test]
    fn the_part_seen_is_the_half_on_that_side_of_the_picture() {
        let (state, set, viewport) = solids();
        let seen = |mm: f64| {
            let at = right_of_the_middle(&state, viewport, mm);
            part_seen(&state, &set, viewport, at).map(|(part, _)| part)
        };
        // From the front the patient's left, part 1, is on the right of the picture.
        assert_eq!((seen(0.6), seen(-0.6)), (Some(1), Some(0)));
        assert_eq!(seen(40.0), None, "beside the solids");
        let (_, place) = part_seen(
            &state,
            &set,
            viewport,
            right_of_the_middle(&state, viewport, 0.6),
        )
        .expect("a part");
        // The line of sight is walked in steps of half the finest voxel, 0.9 mm here.
        assert!(
            (place.y + 0.45).abs() < 1.0,
            "within a step of the face nearest the viewer: {place:?}"
        );
    }

    #[test]
    fn what_is_hidden_or_not_drawn_as_solids_is_not_seen() {
        let (mut state, set, viewport) = solids();
        let at = right_of_the_middle(&state, viewport, 0.6);
        reduce(
            &mut state,
            Action::PartShow {
                part: 1,
                shown: false,
            },
        );
        assert_eq!(
            part_seen(&state, &set, viewport, at),
            None,
            "a hidden part is not there"
        );
        reduce(&mut state, Action::PartsShowAll);
        reduce(&mut state, Action::SceneLook(Look::Surface));
        assert_eq!(
            part_seen(&state, &set, viewport, at),
            None,
            "in another look"
        );
        reduce(&mut state, Action::SceneLook(Look::Parts));
        state.labels.study = Some(voxa_core::StudyId("another".to_owned()));
        assert!(
            !drawn(&state),
            "labels of another study are not drawn as solids here"
        );
        assert_eq!(part_seen(&state, &set, viewport, at), None);
    }

    #[test]
    fn a_click_selects_the_part_clicked_only_while_navigating_in_the_parts_look() {
        let (mut state, _, _) = solids();
        assert_eq!(
            selected_by(&state, Some(1)),
            Some(Action::PartSelect(Some(1)))
        );
        assert_eq!(
            selected_by(&state, None),
            Some(Action::PartSelect(None)),
            "on nothing"
        );
        reduce(&mut state, Action::SetTool(Tool::Marker));
        assert_eq!(
            selected_by(&state, Some(1)),
            None,
            "a marker is being placed"
        );
        reduce(&mut state, Action::SetTool(Tool::Navigate));
        reduce(&mut state, Action::SceneLook(Look::Mip));
        assert_eq!(selected_by(&state, Some(1)), None, "no solids to click on");
    }

    #[test]
    fn the_scene_is_drawn_from_the_look_that_can_be_drawn() {
        let (mut state, _, _) = solids();
        assert_eq!(
            scene_params(&state, 1.0).map(|scene| scene.look),
            Some(Look::Parts)
        );
        reduce(&mut state, Action::CloseLabels);
        let drawn_as = scene_params(&state, 1.0).map(|scene| scene.look);
        assert_eq!(
            drawn_as,
            Some(Look::Surface),
            "with the labels closed, a surface"
        );
        assert!(CAPTION.contains("labels made elsewhere") && CAPTION.contains("did not find"));
    }
}
