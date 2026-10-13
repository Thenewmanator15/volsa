//! Which volume of a series is shown, and the two buttons that step between its volumes.

use egui::Ui;
use voxa_core::SeriesInfo;

use crate::action::Action;
use crate::state::AppState;

/// Which volume this is of how many, and what sets it apart when the files say:
/// "Volume 2 of 5 · b 1000". `None` for a series of one volume.
pub fn label(info: &SeriesInfo) -> Option<String> {
    let set = info.set.as_ref()?;
    let place = format!("Volume {} of {}", set.at + 1, set.count);
    Some(if set.what.is_empty() {
        place
    } else {
        format!("{place} · {}", set.what)
    })
}

/// In the toolbar: the volume the active viewport shows, with a button either side to step to
/// the one before and the one after. Nothing unless that viewport shows a series of several.
pub fn stepper(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) {
    let viewport = state.active;
    let shown = state.viewports.get(viewport);
    let listed = shown
        .and_then(|shown| shown.series.as_ref())
        .and_then(|series| state.series(series));
    let Some(info) = listed.map(|listed| &listed.info) else {
        return;
    };
    let (Some(set), Some(label)) = (&info.set, label(info)) else {
        return;
    };
    ui.separator();
    let before = egui::Button::new("◀");
    let after = egui::Button::new("▶");
    let back = ui.add_enabled(set.at > 0, before);
    ui.label(label);
    let on = ui.add_enabled(set.at + 1 < set.count, after);
    if back.on_hover_text("The volume before ([)").clicked() {
        actions.push(Action::StepVolume { viewport, by: -1 });
    }
    if on.on_hover_text("The volume after (])").clicked() {
        actions.push(Action::StepVolume { viewport, by: 1 });
    }
}

#[cfg(test)]
mod tests {
    use voxa_core::{SeriesId, VolumeSet};

    use super::*;

    fn volume(at: usize, count: usize, what: &str) -> SeriesInfo {
        SeriesInfo {
            set: Some(VolumeSet {
                of: SeriesId("whole".to_owned()),
                at,
                count,
                what: what.to_owned(),
            }),
            ..SeriesInfo::default()
        }
    }

    #[test]
    fn a_volume_says_which_it_is_and_what_sets_it_apart() {
        assert_eq!(
            label(&volume(1, 5, "b 850")).as_deref(),
            Some("Volume 2 of 5 · b 850")
        );
        assert_eq!(label(&volume(0, 3, "")).as_deref(), Some("Volume 1 of 3"));
        assert_eq!(label(&SeriesInfo::default()), None);
    }
}
