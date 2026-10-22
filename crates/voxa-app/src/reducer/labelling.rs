//! Actions about labelling a scan with Voxa's own networks: what is chosen, what is running,
//! what came back, and the result kept for the scene's series.

use voxa_models::Card;
use voxa_segment::Stage;

use super::labels;
use crate::action::{Action, Effect};
use crate::choices::{Choices, RunsOnKept};
use crate::labelling::{Labelling, LabellingAction, Ready, can_label, wanted};
use crate::labels::{LabelsStatus, study_of};
use crate::state::{AppState, Notice, NoticeKind};

/// The key of the notice that says a run was stopped or failed. A new run takes it away.
const LABELLING: &str = "labelling";
/// What is said when a run is stopped.
const STOPPED: &str = "Labelling was stopped.";

/// Apply an action about labelling a scan.
pub(super) fn reduce(state: &mut AppState, action: LabellingAction) -> Vec<Effect> {
    // A choice is asked to be kept as it is made, whatever else it does.
    let chosen = matches!(
        action,
        LabellingAction::SetModelChoice(_)
            | LabellingAction::SetCortex(_)
            | LabellingAction::SetRunsOn(_)
            | LabellingAction::SetKeep(_)
    );
    let labelling = &mut state.labelling;
    match action {
        LabellingAction::LabelScan { held } => return start(state, held),
        LabellingAction::StopLabelling => return stop(state).into_iter().collect(),
        LabellingAction::Look => return look_again(labelling),
        LabellingAction::LabellingStage { run, stage } if waiting_for(labelling, run) => {
            labelling.running = Some(stage);
        }
        LabellingAction::LabellingFailed { run, why } if waiting_for(labelling, run) => {
            failed(state, why);
        }
        LabellingAction::LabellingLooked { look, ready, cards } if look == labelling.looks => {
            looked(labelling, ready, cards);
        }
        // Old news: what a run says that was stopped or overtaken, and what a look found that
        // was asked for before the folders last changed.
        LabellingAction::LabellingStage { .. }
        | LabellingAction::LabellingFailed { .. }
        | LabellingAction::LabellingLooked { .. } => {}
        LabellingAction::DrawingOn(name) => {
            labelling.drawing_on = Some(name).filter(|name| !name.is_empty());
        }
        LabellingAction::SetModelChoice(choice) => labelling.choice = choice,
        LabellingAction::SetCortex(cortex) => labelling.cortex = cortex,
        LabellingAction::SetRunsOn(runs_on) => {
            // A place chosen outright takes the place of a card kept by its name and not yet
            // looked for.
            labelling.runs_on = runs_on;
            labelling.card_kept = None;
        }
        LabellingAction::SetKeep(keep) => labelling.keep = keep,
        LabellingAction::SetModelsFolder(folder) => {
            labelling.models = Some(folder);
            return kept_and_looked(labelling);
        }
        LabellingAction::SetRuntimeFolder(folder) => {
            labelling.runtime = Some(folder);
            return kept_and_looked(labelling);
        }
        about_kept @ (LabellingAction::KeptFound { .. }
        | LabellingAction::ShowKept
        | LabellingAction::DeleteKept
        | LabellingAction::SaveLabelsAs(_)) => return about_kept_results(state, about_kept),
    }
    if chosen {
        vec![Effect::StoreChoices(Choices::of(&state.labelling))]
    } else {
        Vec::new()
    }
}

/// The labelling state with nothing running and nothing kept, which is what another export
/// starts from. What the person chose and what was found of the computer stays.
pub(super) fn emptied(labelling: &Labelling) -> Labelling {
    Labelling {
        running: None,
        on: None,
        kept: None,
        kept_files: None,
        ..labelling.clone()
    }
}

/// Forget what was found kept for the series the scene showed: it is not another's.
pub(super) fn forget_kept(labelling: &mut Labelling) {
    labelling.kept = None;
    labelling.kept_files = None;
}

/// Take in what a look found of this computer and its cards. A card kept by its name from the
/// run before is looked for among them, once; where none has that name the place is left to
/// Voxa.
fn looked(labelling: &mut Labelling, ready: Ready, cards: Vec<Card>) {
    labelling.ready = ready;
    labelling.cards = cards;
    if let Some(name) = labelling.card_kept.take() {
        labelling.runs_on = RunsOnKept::Card(name).find(&labelling.cards);
    }
}

/// Start a run on the series in the scene, if `can_label` allows one. When it does not,
/// nothing changes and nothing is asked for.
fn start(state: &mut AppState, held: u64) -> Vec<Effect> {
    let series = state.scene.series.clone();
    let (Some(series), Some(wanted), Ok(())) = (series, wanted(state), can_label(state)) else {
        return Vec::new();
    };
    // What was said of the run before is old news now.
    super::post(state, LABELLING, None);
    let labelling = &mut state.labelling;
    labelling.running = Some(Stage::Preparing);
    labelling.on = Some(series.clone());
    labelling.runs += 1;
    vec![Effect::LabelScan {
        run: labelling.runs,
        series,
        wanted,
        runs_on: labelling.runs_on,
        held,
        keep: labelling.keep,
        drawing_on: labelling.drawing_on.clone(),
    }]
}

/// Whether the run numbered `run` is the one on: news of any other is old news.
pub(super) fn waiting_for(labelling: &Labelling, run: u64) -> bool {
    labelling.running.is_some() && labelling.runs == run
}

/// The run on failed: it is over, and the sentence is posted as an error.
fn failed(state: &mut AppState, why: String) {
    end_run(&mut state.labelling);
    let notice = Notice {
        kind: NoticeKind::Error,
        text: why,
        details: Vec::new(),
        key: Some(LABELLING),
    };
    super::post(state, LABELLING, Some(notice));
}

/// Clear the run.
fn end_run(labelling: &mut Labelling) {
    labelling.running = None;
    labelling.on = None;
}

/// Stop the run if one is on and say so, asking the worker to stop too. With no run on it
/// does nothing and says nothing.
pub(super) fn stop(state: &mut AppState) -> Option<Effect> {
    state.labelling.running.as_ref()?;
    end_run(&mut state.labelling);
    let notice = Notice {
        kind: NoticeKind::Info,
        text: STOPPED.to_owned(),
        details: Vec::new(),
        key: Some(LABELLING),
    };
    super::post(state, LABELLING, Some(notice));
    Some(Effect::StopLabelling)
}

/// Keep a run on the series it was started on. When the scene has moved to another series, the
/// worker ends the run too (the volume it was given is no longer the one kept), so the run is
/// ended here at once and the worker is asked to drop it for certain. The person moved on
/// themselves, so nothing is said, and what the worker says of the run after is old news. A
/// scene that shows nothing leaves the run alone: its volume is still kept, and the labels
/// wait for the series they are of.
pub(super) fn keep(state: &mut AppState) -> Option<Effect> {
    let moved = state.scene.series.is_some() && state.scene.series != state.labelling.on;
    if state.labelling.running.is_none() || !moved {
        return None;
    }
    end_run(&mut state.labelling);
    Some(Effect::StopLabelling)
}

/// Ask for another look at the computer, in the folders as they are now. Nothing is known of
/// it until the answer comes.
fn look_again(labelling: &mut Labelling) -> Vec<Effect> {
    labelling.ready = Ready::Unknown;
    labelling.looks += 1;
    vec![Effect::LookForLabelling {
        look: labelling.looks,
        models: labelling.models.clone(),
        runtime: labelling.runtime.clone(),
    }]
}

/// A folder was chosen: ask for the choices to be kept, and for another look.
fn kept_and_looked(labelling: &mut Labelling) -> Vec<Effect> {
    let kept = Effect::StoreChoices(Choices::of(labelling));
    std::iter::once(kept).chain(look_again(labelling)).collect()
}

/// Apply an action about the result kept for the scene's series (found, shown, deleted) or
/// about saving the labels that are open.
fn about_kept_results(state: &mut AppState, action: LabellingAction) -> Vec<Effect> {
    let series = state.scene.series.clone();
    match action {
        // A result found for a series the scene no longer shows is old news.
        LabellingAction::KeptFound {
            series: found,
            when,
            labels,
            table,
        } if series.as_ref() == Some(&found) => {
            state.labelling.kept = Some(when);
            state.labelling.kept_files = Some((labels, table));
            Vec::new()
        }
        LabellingAction::ShowKept => match state.labelling.kept_files.clone() {
            Some((labels, table)) => labels::open(state, labels, Some(table)),
            None => Vec::new(),
        },
        LabellingAction::DeleteKept => {
            let was_kept = state.labelling.kept.is_some();
            forget_kept(&mut state.labelling);
            let forget = series.filter(|_| was_kept);
            forget
                .map(|series| Effect::ForgetKept { series })
                .into_iter()
                .collect()
        }
        // Only labels that are open are saved, not ones still being read.
        LabellingAction::SaveLabelsAs(to) if state.labels.status == LabelsStatus::Ready => {
            vec![Effect::SaveLabels { to }]
        }
        _ => Vec::new(),
    }
}

/// Take in the labels a run has made, as the labels of the study of the series it labelled.
/// They are dropped when the scene has moved on to another study; when it shows nothing they
/// are kept, to be shown once the scene is back on their study. The run is over either way.
/// The workers are asked to hold the labels, and so to draw them, only once they are taken in
/// here: until then they draw the labels before, which are the ones listed. Labels that are
/// not taken are let go.
pub(super) fn result(state: &mut AppState, read: Action) -> Vec<Effect> {
    let Action::LabelsRead { run: Some(run), .. } = read else {
        return Vec::new();
    };
    let on = state.labelling.on.clone();
    end_run(&mut state.labelling);
    let study = on.and_then(|series| study_of(state, &series));
    let now = state
        .scene
        .series
        .as_ref()
        .and_then(|series| study_of(state, series));
    match study {
        Some(study) if now.is_none() || now.as_ref() == Some(&study) => {
            let mut effects = labels::taken_in(state, read, Some(study));
            if state.labels.status == LabelsStatus::Ready {
                effects.push(Effect::TakeLabels { run });
            }
            effects
        }
        // The worker is told the run is over, which lets go of what it made.
        _ => vec![Effect::StopLabelling],
    }
}
