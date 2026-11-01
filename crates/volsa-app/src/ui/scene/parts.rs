//! The Parts look in the scene: which labelled part is under a point of the picture, and
//! what a click on it asks for.

use volsa_core::{Patient, Point};
use volsa_labels::LabelSet;
use volsa_render::{Look, pick_part};

use super::scene_params;
use crate::action::Action;
use crate::annotations::Tool;
use crate::labels::look_drawn;
use crate::state::AppState;

/// What the scene says of itself in the Parts look.
pub const CAPTION: &str =
    "Solids are labels made elsewhere, drawn as given: Volsa did not find them in this scan.";

/// A part at least this solid is what the pointer is on. A fainter one is seen through:
/// the pointer is on the first solid part behind it, if there is one.
pub const SOLID_ENOUGH: f32 = 0.5;

/// Whether the scene is drawn as labelled solids now.
pub fn drawn(state: &AppState) -> bool {
    look_drawn(state) == Look::Parts
}

/// The part shown at a point of the scene's picture, given in points from its top left
/// corner, and the point of the patient there: the first shown part on that line of sight
/// that is [`SOLID_ENOUGH`], in what the cuts leave, or the first shown part of any kind
/// where none is. `None` in any other look, and where the picture shows no part.
pub fn part_seen(
    state: &AppState,
    set: &LabelSet,
    viewport: [f32; 2],
    at: [f32; 2],
) -> Option<(usize, Point<Patient>)> {
    if !drawn(state) {
        return None;
    }
    let scene = scene_params(state, 1.0)?;
    let at_least = |solid: f32| -> Vec<bool> {
        let parts = state.labels.shown.iter();
        parts.map(|how| how.shown && how.opacity >= solid).collect()
    };
    // What is solid enough first; a faded part only where nothing solid lies behind it.
    let solid = pick_part(set, &at_least(SOLID_ENOUGH), &scene, viewport, at);
    solid.or_else(|| pick_part(set, &at_least(0.0), &scene, viewport, at))
}

/// What a click in the scene asks of the labels: in the Parts look, with the navigating
/// tool, to select the part clicked, or none where there is none. The same click moves the
/// position: one click means one point.
pub fn selected_by(state: &AppState, part: Option<usize>) -> Option<Action> {
    (drawn(state) && state.tool == Tool::Navigate).then_some(Action::PartSelect(part))
}

#[cfg(test)]
mod tests {
    use volsa_render::Look;

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
        state.labels.study = Some(volsa_core::StudyId("another".to_owned()));
        assert!(
            !drawn(&state),
            "labels of another study are not drawn as solids here"
        );
        assert_eq!(part_seen(&state, &set, viewport, at), None);
    }

    #[test]
    fn a_faded_part_does_not_take_the_pointer_from_a_solid_one_seen_through_it() {
        let (mut state, set, viewport) = solids();
        // From the patient's left, the left half, part 1, is in front of the right, part 0.
        reduce(&mut state, Action::SceneView(volsa_render::NamedView::Left));
        let middle = [viewport[0] / 2.0, viewport[1] / 2.0];
        let seen =
            |state: &AppState| part_seen(state, &set, viewport, middle).map(|(part, _)| part);
        assert_eq!(seen(&state), Some(1), "the one in front");
        let fade = |part, opacity| Action::PartOpacity { part, opacity };
        reduce(&mut state, fade(1, 0.3));
        assert_eq!(seen(&state), Some(0), "what is seen through the faded one");
        reduce(&mut state, fade(1, SOLID_ENOUGH));
        assert_eq!(
            seen(&state),
            Some(1),
            "solid enough to be what is pointed at"
        );
        // With nothing solid behind it, a faded part is still there to point at.
        reduce(&mut state, fade(1, 0.3));
        reduce(&mut state, fade(0, 0.3));
        assert_eq!(seen(&state), Some(1), "both faded: the first met");
        reduce(
            &mut state,
            Action::PartShow {
                part: 0,
                shown: false,
            },
        );
        assert_eq!(seen(&state), Some(1), "alone, and faded");
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
