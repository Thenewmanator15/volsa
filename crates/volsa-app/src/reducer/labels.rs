//! Actions about labelled parts: opening and closing labels, and what is shown of them.

use voxa_core::StudyId;
use voxa_labels::boxes_meet;

use super::labelling;
use crate::action::{Action, Effect};
use crate::labels::{
    FAINTEST, Labels, LabelsStatus, NOTHING_TO_LABEL, NOTHING_TO_LABEL_KEY, NOWHERE_NEAR,
    PartShown, on_this_series, study_of,
};
use crate::planes::grid;
use crate::state::{AppState, Notice, NoticeKind};

const NOTHING_TO_PUT_THEM_ON: &str = NOTHING_TO_LABEL_KEY;

/// Whether an action is about labels.
pub(super) fn concerns(action: &Action) -> bool {
    matches!(
        action,
        Action::OpenLabels { .. }
            | Action::LabelsRead { .. }
            | Action::LabelsFailed(_)
            | Action::CloseLabels
            | Action::PartShow { .. }
            | Action::GroupShow { .. }
            | Action::PartsShowAll
            | Action::PartIsolate(_)
            | Action::PartOpacity { .. }
            | Action::PartSelect(_)
            | Action::SetOverlay(_)
    )
}

/// Apply an action about labels. Whatever it refers to that is not there is ignored.
pub(super) fn reduce(state: &mut AppState, action: Action) -> Vec<Effect> {
    match action {
        Action::OpenLabels { labels, table } => return open(state, labels, table),
        Action::CloseLabels => {
            state.labels = emptied(&state.labels);
            let stopped = labelling::stop(state);
            return stopped.into_iter().chain([Effect::DropLabels]).collect();
        }
        Action::LabelsFailed(why) if state.labels.status == LabelsStatus::Reading => {
            state.labels.status = LabelsStatus::Failed(why);
        }
        // What a run of labelling brings back is not a file that was read, and it is taken
        // only from the run that is on. Any other run's is old news.
        read @ Action::LabelsRead { run: Some(run), .. } => {
            if labelling::waiting_for(&state.labelling, run) {
                return labelling::result(state, read);
            }
        }
        read @ Action::LabelsRead { run: None, .. }
            if state.labels.status == LabelsStatus::Reading =>
        {
            let study = state.labels.study.clone();
            return taken_in(state, read, study);
        }
        Action::SetOverlay(overlay) => state.labels.overlay = overlay,
        about_a_part => shown(&mut state.labels, &about_a_part),
    }
    Vec::new()
}

/// No labels, drawn over the planes the way these were.
pub(super) fn emptied(labels: &Labels) -> Labels {
    Labels {
        overlay: labels.overlay,
        ..Labels::default()
    }
}

/// Ask for a label file to be read. Labels are put on a volume, so there must be one ready;
/// they will belong to the study of the series it is of. With no volume nothing is read, and
/// a notice says what to do first. A run of labelling that is on is stopped: the labels it
/// would bring back would take the place of these.
pub(super) fn open(
    state: &mut AppState,
    labels: std::path::PathBuf,
    table: Option<std::path::PathBuf>,
) -> Vec<Effect> {
    let series = state.scene.series.clone().filter(|_| grid(state).is_some());
    let Some(study) = series.and_then(|series| study_of(state, &series)) else {
        let notice = Notice {
            kind: NoticeKind::Warning,
            text: NOTHING_TO_LABEL.to_owned(),
            details: Vec::new(),
            key: Some(NOTHING_TO_PUT_THEM_ON),
        };
        super::post(state, NOTHING_TO_PUT_THEM_ON, Some(notice));
        return Vec::new();
    };
    super::post(state, NOTHING_TO_PUT_THEM_ON, None);
    state.labels = Labels {
        status: LabelsStatus::Reading,
        study: Some(study),
        ..emptied(&state.labels)
    };
    let stopped = labelling::stop(state);
    let read = Effect::ReadLabels { labels, table };
    stopped.into_iter().chain([read]).collect()
}

/// Take in labels a worker has made or read, as the labels of `study`: every part shown,
/// solid. If the scene shows a series of that study and their box lies nowhere near its
/// volume, they are another scan's and are refused.
pub(super) fn taken_in(state: &mut AppState, read: Action, study: Option<StudyId>) -> Vec<Effect> {
    let Action::LabelsRead {
        title,
        parts,
        verdict,
        corners,
        names_from,
        made,
        run: _,
    } = read
    else {
        return Vec::new();
    };
    let scene = state.scene.series.as_ref();
    let of_this_study = scene.and_then(|series| study_of(state, series)) == study;
    let volume = state.scene.corners.filter(|_| of_this_study);
    if volume.is_some_and(|volume| !boxes_meet(&corners, &volume)) {
        state.labels = Labels {
            status: LabelsStatus::Failed(NOWHERE_NEAR.to_owned()),
            ..emptied(&state.labels)
        };
        return vec![Effect::DropLabels];
    }
    let solid = PartShown {
        shown: true,
        opacity: 1.0,
    };
    state.labels = Labels {
        status: LabelsStatus::Ready,
        title,
        shown: vec![solid; parts.len()],
        parts,
        selected: None,
        overlay: state.labels.overlay,
        verdict,
        study,
        corners: Some(corners),
        names_from,
        made,
    };
    Vec::new()
}

/// Change what is shown of the labels, how solid, and which part is selected. A part that
/// is hidden is no longer selected.
fn shown(labels: &mut Labels, action: &Action) {
    match action {
        Action::PartShow { part, shown } => {
            if let Some(how) = labels.shown.get_mut(*part) {
                how.shown = *shown;
            }
        }
        Action::GroupShow { group, shown } => {
            let of_the_group = labels.parts.iter().map(|part| part.group == *group);
            for (how, _) in labels
                .shown
                .iter_mut()
                .zip(of_the_group)
                .filter(|(_, of)| *of)
            {
                how.shown = *shown;
            }
        }
        Action::PartsShowAll => labels.shown.iter_mut().for_each(|how| how.shown = true),
        Action::PartIsolate(only) if *only < labels.shown.len() => {
            for (place, how) in labels.shown.iter_mut().enumerate() {
                how.shown = place == *only;
            }
        }
        Action::PartOpacity { part, opacity } if opacity.is_finite() => {
            if let Some(how) = labels.shown.get_mut(*part) {
                how.opacity = opacity.clamp(FAINTEST, 1.0);
            }
        }
        Action::PartSelect(None) => labels.selected = None,
        Action::PartSelect(Some(part)) if labels.is_shown(*part) => labels.selected = Some(*part),
        _ => {}
    }
    labels.selected = labels.selected.filter(|part| labels.is_shown(*part));
}

/// Whether the Parts look can be taken up: only with labels on the series in the scene.
pub(super) fn parts_look_allowed(state: &AppState) -> bool {
    on_this_series(state)
}
