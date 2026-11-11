//! Actions about measurements and markers, and keeping them with the series they were made
//! on.

use voxa_core::SeriesId;

use super::post;
use crate::action::{Action, Effect};
use crate::annotations::{Annotation, Annotations, MAX_ANNOTATIONS, Tool};
use crate::planes::grid;
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
/// while there is a volume ready to measure on. Every change to the list asks for the list
/// to be kept.
pub(super) fn reduce(state: &mut AppState, action: Action) -> Vec<Effect> {
    let measurable = grid(state).is_some();
    let list = &mut state.annotations;
    let changed = match action {
        Action::SetTool(tool) if measurable => {
            state.tool = tool;
            false
        }
        Action::AnnotationAdd(shape) if measurable => {
            let added = list.add(shape);
            if !added {
                post(state, MEASUREMENTS, Some(full()));
            }
            added
        }
        Action::AnnotationRemove(id) => list.remove(id),
        Action::AnnotationRename { id, label } => list.rename(id, &label),
        Action::AnnotationsClear => list.clear(),
        Action::Undo => list.undo(),
        Action::Redo => list.redo(),
        Action::AnnotationsLoaded { series, items } => {
            read(state, &series, items);
            false
        }
        Action::AnnotationsNotStored { reason } => {
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

/// What is said when the annotations could not be written to where they are kept.
fn not_kept(reason: String) -> Notice {
    Notice {
        kind: NoticeKind::Warning,
        text: "Measurements could not be written, so they will not be there next time".to_owned(),
        details: vec![reason],
        key: Some(NOT_KEPT),
    }
}
