//! Actions about measurements and markers, and keeping them with the series they were made
//! on.

use voxa_core::SeriesId;

use super::post;
use crate::action::{Action, Effect};
use crate::annotations::{Annotation, Annotations, MAX_ANNOTATIONS, Tool};
use crate::measuring::{scale_now, why_not_measurable};
use crate::state::{AppState, Notice, NoticeKind};

/// The key of the notice that says the list of measurements is full.
const MEASUREMENTS: &str = "measurements";
/// The key of the notice that says annotations could not be written.
const NOT_KEPT: &str = "annotations";

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
            | Action::AnnotationsLoaded { .. }
            | Action::AnnotationsNotStored { .. }
    )
}

/// Apply an action about annotations. A tool is taken up, and an annotation added, only
/// while there is something to measure on: a volume ready, or a slice shown whose files give
/// it a scale. What is added is kept with that scale. Every change to the list asks for the
/// list to be kept.
pub(super) fn reduce(state: &mut AppState, action: Action) -> Vec<Effect> {
    let scale = scale_now(state);
    let list = &mut state.annotations;
    let changed = match (action, scale) {
        (Action::SetTool(tool), Some(_)) => {
            state.tool = tool;
            false
        }
        (Action::AnnotationAdd(shape), Some(scale)) => {
            let added = list.add(shape, scale);
            if !added {
                post(state, MEASUREMENTS, Some(full()));
            }
            added
        }
        (Action::AnnotationRemove(id), _) => list.remove(id),
        (Action::AnnotationRename { id, label }, _) => list.rename(id, &label),
        (Action::AnnotationsClear, _) => list.clear(),
        (Action::Undo, _) => list.undo(),
        (Action::Redo, _) => list.redo(),
        (Action::AnnotationsLoaded { series, items }, _) => {
            read(state, &series, items);
            false
        }
        (Action::AnnotationsNotStored { reason }, _) => {
            post(state, NOT_KEPT, Some(not_kept(reason)));
            false
        }
        _ => false,
    };
    if changed { kept(state) } else { Vec::new() }
}

/// Ask for the list as it now is to be written where it is kept.
fn kept(state: &AppState) -> Vec<Effect> {
    let items = state.annotations.items.clone();
    let series = state.scene.series.clone();
    let store = series.map(|series| Effect::StoreAnnotations { series, items });
    store.into_iter().collect()
}

/// Take in the annotations read for a series: they become the list, with nothing to undo,
/// if the scene still shows that series and the person has not changed the list meanwhile.
fn read(state: &mut AppState, series: &SeriesId, mut items: Vec<Annotation>) {
    let list = &state.annotations;
    let untouched = list.items.is_empty() && !list.can_undo() && !list.can_redo();
    if state.scene.series.as_ref() == Some(series) && untouched {
        items.truncate(MAX_ANNOTATIONS);
        state.annotations = Annotations::loaded(items);
    }
}

/// Keep the annotations with their series: when the scene has moved to another series than
/// `was`, the list and its history are put away, to be taken back if the scene comes back to
/// that series in this run, and the series it has come to has the list it was left with or a
/// fresh one; the tool is put down And put it down
/// whenever there is nothing to use it on: no volume ready, and no slice shown with a scale.
pub(super) fn settle(state: &mut AppState, was: Option<&SeriesId>) {
    if state.scene.series.as_ref() != was {
        // What was done to the list of the series left can be undone when the series is back.
        let left = std::mem::take(&mut state.annotations);
        if let Some(was) = was {
            state.annotations_left.put_away(was.clone(), left);
        }
        let arrived = state.scene.series.clone();
        let back = arrived.and_then(|series| state.annotations_left.take_back(&series));
        state.annotations = back.unwrap_or_default();
        state.tool = Tool::Navigate;
    }
    if why_not_measurable(state).is_some() {
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

/// What is said when the annotations could not be written to where they are kept.
fn not_kept(reason: String) -> Notice {
    Notice {
        kind: NoticeKind::Warning,
        text: "Measurements could not be written, so they will not be there next time".to_owned(),
        details: vec![reason],
        key: Some(NOT_KEPT),
    }
}
