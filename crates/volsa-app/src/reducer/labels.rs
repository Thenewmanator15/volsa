//! Actions about labelled parts: opening and closing labels, and what is shown of them.

use volsa_core::StudyId;
use volsa_labels::boxes_meet;

use super::labelling;
use crate::action::{Action, Effect};
use crate::labels::{
    FAINTEST, Labels, LabelsStatus, NOTHING_TO_LABEL, NOTHING_TO_LABEL_KEY, NOWHERE_NEAR,
    OTHER_STUDY, PartShown, on_this_series, study_of,
};
use crate::naming::plain_name;
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
    reading(state, |_| Effect::ReadLabels { labels, table })
}

/// Ask for the segmentation (DICOM SEG) in `entry`, of the study `study_uid`, to be read and
/// taken in as labels, as a label file is (see [`open`]). One made on another study than the
/// scan in the scene is not read, and a notice says so. Once it is being read the document is
/// closed, so that the labels are seen.
pub(super) fn open_segmentation(
    state: &mut AppState,
    entry: String,
    study_uid: &str,
) -> Vec<Effect> {
    let scene = state.scene.series.as_ref();
    let study = scene.and_then(|series| study_of(state, series));
    if study.is_some_and(|study| study.0 != study_uid) {
        let notice = Notice {
            kind: NoticeKind::Warning,
            text: OTHER_STUDY.to_owned(),
            details: Vec::new(),
            key: Some(NOTHING_TO_PUT_THEM_ON),
        };
        super::post(state, NOTHING_TO_PUT_THEM_ON, Some(notice));
        return Vec::new();
    }
    let effects = reading(state, |_| Effect::ReadSegmentation { entry });
    if !effects.is_empty() {
        state.document = None;
    }
    effects
}

/// Ask for the thin dark tubes of the scan in the scene to be found, and taken in as its
/// labels, as a label file is read (see [`open`]).
pub(super) fn find_veins(state: &mut AppState) -> Vec<Effect> {
    // Named as the series list names it, which knows makers' own words such as GE's eSWAN.
    let named = |state: &AppState, series: &volsa_core::SeriesId| {
        let name = state.series(series).map(|listed| plain_name(&listed.info));
        name.is_some_and(|name| name.contains("SWI") || name.contains("T2*"))
    };
    let series = state.scene.series.clone();
    let named = series.as_ref().is_some_and(|series| named(state, series));
    // Labels open on this study are kept, and the tubes put onto them as one more part.
    let onto = on_this_series(state).then_some(state.labels.names_from);
    reading(state, |series| Effect::FindVeins {
        series,
        named,
        onto,
    })
}

/// Start labels being read, or made, for the series in the scene, by the effect `read` gives
/// for it; or say that there is no volume to put them on.
fn reading(state: &mut AppState, read: impl FnOnce(volsa_core::SeriesId) -> Effect) -> Vec<Effect> {
    let series = state.scene.series.clone().filter(|_| grid(state).is_some());
    let study = series.as_ref().and_then(|series| study_of(state, series));
    let (Some(series), Some(study)) = (series, study) else {
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
    stopped.into_iter().chain([read(series)]).collect()
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
    // Parts start a little see-through, so that those inside others are seen in 3D; the veins
    // found, drawn through the rest, start fainter still so that the brain shows behind them.
    let start = |part: &volsa_labels::Part| PartShown {
        shown: true,
        opacity: if part.name == volsa_segment::VEINS_PART {
            crate::labels::VEINS_AT_FIRST
        } else {
            crate::labels::PARTS_AT_FIRST
        },
    };
    state.labels = Labels {
        status: LabelsStatus::Ready,
        title,
        shown: parts.iter().map(start).collect(),
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
