//! Actions about the saved views of the scene's series.

use super::{VIEWS, post};
use crate::action::Effect;
use crate::scene::SceneStatus;
use crate::state::{AppState, Notice, NoticeKind};
use crate::views::{MAX_VIEWS, SavedView, ViewsAction};

pub(super) fn reduce(state: &mut AppState, action: ViewsAction) -> Vec<Effect> {
    match action {
        ViewsAction::Save { name } => return save(state, &name),
        ViewsAction::Apply(at) => apply(state, at),
        ViewsAction::Delete(at) if at < state.views.len() => {
            state.views.remove(at);
            return store(state);
        }
        ViewsAction::Loaded { series, views } if state.scene.series.as_ref() == Some(&series) => {
            take_kept(state, views);
        }
        ViewsAction::NotStored { reason } => {
            let notice = Notice {
                kind: NoticeKind::Warning,
                text: "Saved views could not be written, so they will not be there next time"
                    .to_owned(),
                details: vec![reason],
                key: Some(VIEWS),
            };
            post(state, VIEWS, Some(notice));
        }
        _ => {}
    }
    Vec::new()
}

/// Take the views kept from before for the scene's series, as read from their file. A view
/// saved while the file was being read is not lost to them: it comes after them, unless the
/// file was read late enough to hold it already. Past the most that are kept, the oldest go.
fn take_kept(state: &mut AppState, mut kept: Vec<SavedView>) {
    kept.truncate(MAX_VIEWS);
    for saved in std::mem::take(&mut state.views) {
        if !kept.contains(&saved) {
            kept.push(saved);
        }
    }
    let over = kept.len().saturating_sub(MAX_VIEWS);
    kept.drain(..over);
    state.views = kept;
}

/// Save the scene as it is, seen through the active viewport's window. Only a volume that is
/// there can be saved a view of. Past the most that are kept, the oldest goes.
fn save(state: &mut AppState, name: &str) -> Vec<Effect> {
    let window = state
        .viewports
        .get(state.active)
        .map(|viewport| viewport.wl);
    let (Some(window), SceneStatus::Ready) = (window, &state.scene.status) else {
        return Vec::new();
    };
    let view = SavedView::of(name, state.views.len() + 1, &state.scene, window);
    state.views.push(view);
    if state.views.len() > MAX_VIEWS {
        state.views.remove(0);
    }
    store(state)
}

/// Put the scene, and the window of the active viewport, back as a saved view has them. The
/// camera is then the person's own, as if they had moved it there.
fn apply(state: &mut AppState, at: usize) {
    let Some(view) = state.views.get(at) else {
        return;
    };
    let scene = &mut state.scene;
    scene.camera = view.camera();
    scene.look = view.look();
    scene.cuts = view.cuts();
    scene.untouched = false;
    if let Some(viewport) = state.viewports.get_mut(state.active) {
        viewport.wl = view.window();
        viewport.wl_auto = false;
        viewport.wl_pending = false;
    }
}

/// Ask for the views as they now are to be written to their file.
fn store(state: &AppState) -> Vec<Effect> {
    let series = state.scene.series.clone();
    let views = state.views.clone();
    series
        .map(|series| Effect::StoreViews { series, views })
        .into_iter()
        .collect()
}
