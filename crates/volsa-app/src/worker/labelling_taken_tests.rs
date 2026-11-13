//! The worker that labels a scan: the labels a run made are held, and so drawn, only once the
//! state has taken them in; a run that waits for its turn and is stopped gives up at once;
//! and runs and looks go on threads of their own. Stand-ins only (see
//! `labelling_stand_ins.rs`).

use std::sync::Arc;

use volsa_labels::LabelSet;
use volsa_models::RunsOn;

use super::lab::{Lab, stages};
use super::stand_ins::{PATIENCE, StandIn, one_at_a_time};
use crate::action::{Action, Effect};
use crate::labelling::LabellingAction;

/// The name of the thread a run, or a look, goes on.
const OWN_THREAD: &str = "volsa-labelling";

/// The labels of a first run, not kept, taken in by the state.
fn first_taken(lab: &Lab) -> Arc<LabelSet> {
    lab.label(1, RunsOn::Automatic);
    lab.until_taken();
    lab.workers.labels().expect("held once taken")
}

#[test]
fn the_labels_of_a_run_are_held_only_once_the_state_takes_them() {
    let _alone = one_at_a_time();
    let lab = Lab::new(StandIn::roomy());
    lab.label(1, RunsOn::Automatic);
    let said = lab.until_the_end();
    let Some(Action::LabelsRead {
        parts,
        run: Some(1),
        ..
    }) = said.last()
    else {
        panic!("labels are brought back: {said:?}");
    };
    assert!(
        lab.workers.labels().is_none(),
        "nothing is drawn of them before the state lists them"
    );
    // A word about another run takes nothing in.
    lab.workers.run(Effect::TakeLabels { run: 2 }, None);
    assert!(lab.workers.labels().is_none());
    lab.workers.run(Effect::TakeLabels { run: 1 }, None);
    let held = lab.workers.labels().expect("held once taken");
    assert_eq!(&held.parts, parts);
}

/// What is held after a second run, to be kept, brought back its labels and `meanwhile` was
/// done before the state took them: the state, finding the run over, drops its report and
/// asks the workers to stop labelling, as it does on Stop and when the scene moves on.
fn second_left_untaken(meanwhile: impl FnOnce(&Lab)) {
    let _alone = one_at_a_time();
    let lab = Lab::keeping(StandIn::roomy());
    let first = first_taken(&lab);
    lab.label_keeping(2, RunsOn::Automatic, true);
    let said = lab.until_the_end();
    assert!(
        matches!(said.last(), Some(Action::LabelsRead { run: Some(2), .. })),
        "{said:?}"
    );
    meanwhile(&lab);
    lab.workers.run(Effect::StopLabelling, None);
    lab.settled();
    let held = lab.workers.labels().expect("the labels before");
    assert!(
        Arc::ptr_eq(&held, &first),
        "the labels drawn are still the ones the state lists"
    );
    // A late word about the run takes nothing in.
    lab.workers.run(Effect::TakeLabels { run: 2 }, None);
    let held = lab.workers.labels().expect("the labels before");
    assert!(Arc::ptr_eq(&held, &first));
    assert_eq!(lab.said(), [], "and nothing was kept of the run");
    assert!(
        !lab.settings().join("labels").exists(),
        "nothing was written"
    );
}

#[test]
fn a_run_stopped_between_its_report_and_its_taking_leaves_the_labels_held_as_they_were() {
    second_left_untaken(|_| {});
}

#[test]
fn a_run_whose_series_changes_between_its_report_and_its_taking_leaves_the_labels_as_they_were() {
    second_left_untaken(Lab::another_series);
}

#[test]
fn a_run_stopped_while_it_waits_for_its_turn_gives_up_at_once_and_lets_its_volume_go() {
    let _alone = one_at_a_time();
    let (stand_in, running, go) = StandIn::roomy().held_while_running();
    let lab = Lab::new(stand_in);
    lab.label(1, RunsOn::Automatic);
    running
        .recv_timeout(PATIENCE)
        .expect("the first network runs");
    // The lab, the store and the first run hold the head.
    let before = lab.holders();
    // The second run stops the first, whose network runs on, and waits for its turn.
    lab.label(2, RunsOn::Automatic);
    lab.workers.run(Effect::StopLabelling, None);
    Lab::eventually("the stopped waiter to let the head go", || {
        lab.holders() == before
    });
    go.send(()).expect("the network waits");
    lab.settled();
    let said = lab.said();
    assert_eq!(stages(&said, 2), [], "the second run said nothing");
    let failed = |said: &Action| {
        matches!(
            said,
            Action::Labelling(LabellingAction::LabellingFailed { run: 2, .. })
        )
    };
    assert!(!said.iter().any(failed), "{said:?}");
    assert_eq!(
        lab.stand_in.opened().len(),
        2,
        "the first run's network and the one refused after it; none of the second's"
    );
}

#[test]
fn a_run_and_a_look_go_on_threads_of_their_own_and_not_on_the_workers_pool() {
    let _alone = one_at_a_time();
    let lab = Lab::new(StandIn::roomy());
    assert_eq!(lab.stand_in.looked_on(), [OWN_THREAD]);
    lab.label(1, RunsOn::Automatic);
    lab.until_the_end();
    let threads = lab.stand_in.threads();
    assert_eq!(threads.len(), 4, "{threads:?}");
    assert!(threads.iter().all(|name| name == OWN_THREAD), "{threads:?}");
}

#[test]
fn highlighting_veins_on_a_scan_with_no_tube_says_none_were_found() {
    let _alone = one_at_a_time();
    let lab = Lab::new(StandIn::roomy());
    let series = crate::testing::id(crate::testing::THICK_SCAN);
    let find = Effect::FindVeins {
        series,
        named: false,
    };
    lab.workers.run(find, None);
    // The made-up head is a smooth ramp: it holds no tube, and that is said.
    assert_eq!(
        lab.next(),
        Action::LabelsFailed(volsa_segment::VeinsError::NoneFound.to_string())
    );
    assert!(lab.workers.labels().is_none(), "nothing is held");
}
