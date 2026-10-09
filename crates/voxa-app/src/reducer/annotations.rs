//! Actions about measurements and markers, and keeping them with the series they were made
//! on.

use voxa_core::SeriesId;

use super::post;
use crate::action::Action;
use crate::annotations::{Annotations, MAX_ANNOTATIONS, Tool};
use crate::planes::grid;
use crate::state::{AppState, Notice, NoticeKind};

/// The key of the notice that says the list of measurements is full.
const MEASUREMENTS: &str = "measurements";

/// Whether an action is about annotations or the tool that makes them.
pub(super) fn concerns(action: &Action) -> bool {
    matches!(
        action,
        Action::SetTool(_)
            | Action::AnnotationAdd(_)
            | Action::AnnotationRemove(_)
            | Action::AnnotationRename { .. }
            | Action::AnnotationsClear
            | Action::Undo
            | Action::Redo
    )
}

/// Apply an action about annotations. A tool is taken up, and an annotation added, only
/// while there is a volume ready to measure on.
pub(super) fn reduce(state: &mut AppState, action: Action) {
    let measurable = grid(state).is_some();
    match action {
        Action::SetTool(tool) if measurable => state.tool = tool,
        Action::AnnotationAdd(shape) if measurable => {
            if !state.annotations.add(shape) {
                post(state, MEASUREMENTS, Some(full()));
            }
        }
        Action::AnnotationRemove(id) => {
            state.annotations.remove(id);
        }
        Action::AnnotationRename { id, label } => {
            state.annotations.rename(id, &label);
        }
        Action::AnnotationsClear => {
            state.annotations.clear();
        }
        Action::Undo => {
            state.annotations.undo();
        }
        Action::Redo => {
            state.annotations.redo();
        }
        _ => {}
    }
}

/// Keep the annotations with their series: when the scene has moved to another series than
/// `was`, the list and its history start afresh. And put the tool down whenever there is no
/// volume ready to use it on.
pub(super) fn settle(state: &mut AppState, was: Option<&SeriesId>) {
    if state.scene.series.as_ref() != was {
        state.annotations = Annotations::default();
    }
    if grid(state).is_none() {
        state.tool = Tool::Navigate;
    }
}

/// What is said when one more annotation is asked for than the list holds.
fn full() -> Notice {
    Notice {
        kind: NoticeKind::Warning,
        text: format!(
            "There are already {MAX_ANNOTATIONS} measurements. Remove one to add another."
        ),
        details: Vec::new(),
        key: Some(MEASUREMENTS),
    }
}
