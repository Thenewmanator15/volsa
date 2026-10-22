//! The worker that labels a scan: a run from its first stage to its labels, what it says when
//! a place gives way, and what it does not say once it is stopped, given up or overtaken.
//! Stand-ins only (see `labelling_stand_ins.rs`).

use std::sync::atomic::Ordering;

use voxa_models::{Card, CardId, Device, Role, RunError, RunsOn};
use voxa_segment::Stage;

use super::lab::{Lab, failed_run, made_by_run, stages};
use super::stand_ins::{self, GIB, PATIENCE, StandIn, WHEN, card_a, one_at_a_time};
use crate::action::{Action, Effect};
use crate::labels::NamesFrom;

#[test]
fn a_run_says_its_stages_in_order_then_brings_back_its_parts_and_who_made_them() {
    let _alone = one_at_a_time();
    let lab = Lab::new(StandIn::roomy());
    lab.label(1, RunsOn::Automatic);
    let said = lab.until_taken();
    let finding = |step| Stage::Finding { step, of: 3 };
    let expected = [
        Stage::Preparing,
        finding(1),
        finding(2),
        finding(3),
        Stage::Checking,
        Stage::Tidying,
    ];
    assert_eq!(stages(&said, 1), expected);
    let Some(Action::LabelsRead {
        title,
        parts,
        made,
        run,
        names_from,
        ..
    }) = said.last()
    else {
        panic!("labels are brought back: {said:?}");
    };
    assert_eq!(*run, Some(1));
    assert_eq!(
        *names_from,
        NamesFrom::Model,
        "the names are the model's own"
    );
    let names: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(names, ["structure 1", "structure 2", "structure 3"]);
    let sum = voxa_models::read_model(lab.folder.path())
        .expect("reads")
        .sum;
    let expected = voxa_labels::Made {
        by: format!("Voxa {}", env!("CARGO_PKG_VERSION")),
        model: "SynthSeg 2.0, robust model".to_owned(),
        model_sum: sum,
        device: "Made-up card A".to_owned(),
        when: WHEN.to_owned(),
        notes: Vec::new(),
    };
    assert_eq!(made.as_ref(), Some(&expected));
    assert_eq!(*title, expected.model);
    let kept = lab.workers.labels().expect("the labels are kept");
    assert_eq!((&kept.parts, &kept.made), (parts, made));
    let on_the_card = Device::Card(card_a().id);
    let every = [Role::Robust1, Role::Robust2, Role::Robust3, Role::Scores];
    assert_eq!(lab.stand_in.opened(), every.map(|role| (role, on_the_card)));
    lab.settled();
    assert_eq!(lab.said(), [], "and that is all");
}

#[test]
fn a_card_without_room_gives_way_to_the_processor_and_the_result_says_so() {
    let _alone = one_at_a_time();
    let card = Device::Card(card_a().id);
    let lab = Lab::new(StandIn::roomy().refusing(card, RunError::NoRoom));
    lab.label(1, RunsOn::Automatic);
    let (made, run) = made_by_run(&lab.until_the_end());
    assert_eq!(run, Some(1));
    assert_eq!(made.device, "the processor");
    let note = "Run on the processor: the graphics card did not have room.";
    assert_eq!(made.notes, [note]);
    let processor = Device::Processor;
    let tried = [
        (Role::Robust1, card),
        (Role::Robust1, processor),
        (Role::Robust2, processor),
        (Role::Robust3, processor),
        (Role::Scores, processor),
    ];
    assert_eq!(lab.stand_in.opened(), tried);
}

#[test]
fn a_card_that_stops_answering_is_said_and_is_not_tried_again() {
    let _alone = one_at_a_time();
    let card = Device::Card(card_a().id);
    // The card is lost; the whole scan has no room on the processor; the head does. (The head
    // is tried only where cutting to it makes the image smaller.)
    let stand_in = StandIn::roomy()
        .refusing(card, RunError::DeviceLost)
        .refusing(Device::Processor, RunError::NoRoom);
    let lab = Lab::with_a_long_head(stand_in);
    lab.label(1, RunsOn::Automatic);
    let (made, _) = made_by_run(&lab.until_the_end());
    assert_eq!(made.device, "the processor");
    let note = "Run on the processor: the graphics card stopped answering.";
    assert_eq!(
        made.notes.len(),
        2,
        "the cut, then the fall: {:?}",
        made.notes
    );
    assert!(made.notes[0].starts_with("The scan was cut to the head"));
    assert_eq!(made.notes[1], note);
    let opened = lab.stand_in.opened();
    let on_the_card = opened.iter().filter(|(_, on)| *on == card).count();
    assert_eq!(
        on_the_card, 1,
        "the head is not tried on a card that was lost"
    );
    assert_eq!(opened.len(), 6, "{opened:?}");
}

#[test]
fn a_card_without_room_that_gives_way_to_another_card_is_said() {
    let _alone = one_at_a_time();
    let card_b = Card {
        id: CardId(3),
        name: "Made-up card B".to_owned(),
        ..card_a()
    };
    let cards = vec![card_a(), card_b.clone()];
    let stand_in = StandIn::of(cards, vec![Some(16 * GIB); 2], 64 * GIB)
        .refusing(Device::Card(card_a().id), RunError::NoRoom);
    let lab = Lab::new(stand_in);
    lab.label(1, RunsOn::Automatic);
    let (made, _) = made_by_run(&lab.until_the_end());
    assert_eq!(made.device, "Made-up card B");
    let note = "Run on the graphics card (Made-up card B): the graphics card (Made-up card A) \
                did not have room.";
    assert_eq!(made.notes, [note]);
    let on_b = lab
        .stand_in
        .opened()
        .iter()
        .skip(1)
        .all(|(_, on)| *on == Device::Card(card_b.id));
    assert!(on_b, "{:?}", lab.stand_in.opened());
}

#[test]
fn scores_that_do_not_fit_on_the_card_are_made_on_the_processor_and_it_is_said() {
    let _alone = one_at_a_time();
    // Room on the card for the networks that find the structures, not for the scores one.
    let lab = Lab::new(StandIn::with(6 * GIB, 64 * GIB));
    lab.label(1, RunsOn::Automatic);
    let (made, _) = made_by_run(&lab.until_the_end());
    assert_eq!(made.device, "Made-up card A");
    let note = "The model checked its own work on the processor: that step did not fit on the \
                graphics card.";
    assert_eq!(made.notes, [note]);
    let card = Device::Card(card_a().id);
    let tried = [
        (Role::Robust1, card),
        (Role::Robust2, card),
        (Role::Robust3, card),
        (Role::Scores, Device::Processor),
    ];
    assert_eq!(lab.stand_in.opened(), tried);
}

/// A run whose first network is held while it runs, until `meanwhile` has been done. What the
/// run said before that, and once it has said all it will say.
fn held_run(meanwhile: impl FnOnce(&Lab)) -> (Lab, Vec<Action>) {
    let (stand_in, running, go) = StandIn::roomy().held_while_running();
    let lab = Lab::new(stand_in);
    lab.label(1, RunsOn::Automatic);
    running
        .recv_timeout(PATIENCE)
        .expect("the first network runs");
    meanwhile(&lab);
    go.send(()).expect("the network waits");
    lab.settled();
    let said = lab.said();
    (lab, said)
}

/// Ask the workers for an effect.
fn ask(lab: &Lab, effect: Effect) {
    lab.workers.run(effect, None);
}

/// What a run that is over part-way, after its first network, has said and kept: its first
/// two stages and no more, no labels, and a second network that was refused.
fn assert_over_part_way(lab: &Lab, said: &[Action]) {
    let first = [Stage::Preparing, Stage::Finding { step: 1, of: 3 }];
    assert_eq!(stages(said, 1), first, "nothing more is said");
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(lab.workers.labels().is_none(), "nothing is kept");
    // The network that was running ran to its end; the next was refused.
    let card = Device::Card(card_a().id);
    let opened = [(Role::Robust1, card), (Role::Robust2, card)];
    assert_eq!(lab.stand_in.opened(), opened);
}

#[test]
fn a_run_stopped_after_its_first_stages_says_and_keeps_nothing_more() {
    let (lab, said) = held_run(|lab| ask(lab, Effect::StopLabelling));
    assert_over_part_way(&lab, &said);
}

#[test]
fn labels_given_up_during_a_run_take_its_result_with_them() {
    let _alone = one_at_a_time();
    let (lab, said) = held_run(|lab| ask(lab, Effect::DropLabels));
    assert_eq!(
        stages(&said, 1).len(),
        2,
        "nothing is said once they are given up"
    );
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(lab.workers.labels().is_none());
    // The run was not stopped: it ran to its end, and its result went with the labels.
    assert_eq!(lab.stand_in.opened().len(), 4);
}

#[test]
fn a_network_that_panics_fails_its_run_and_the_next_run_works() {
    let _alone = one_at_a_time();
    let lab = Lab::new(StandIn::roomy());
    lab.stand_in.panics.store(true, Ordering::SeqCst);
    lab.label(1, RunsOn::Automatic);
    let (run, why) = failed_run(&lab.until_the_end());
    assert_eq!(run, 1);
    assert_eq!(
        why,
        "Labelling stopped part-way: something went wrong inside it."
    );
    lab.settled();
    lab.label(2, RunsOn::Automatic);
    let (_, run) = made_by_run(&lab.until_the_end());
    assert_eq!(run, Some(2));
}

#[test]
fn a_network_that_fails_in_its_own_words_is_said_in_plain_words_and_nothing_else_is_tried() {
    let card = Device::Card(card_a().id);
    let words = "Load model from C:\\Users\\Made Up\\models\\standard.onnx failed".to_owned();
    let lab = Lab::new(StandIn::roomy().refusing(card, RunError::Failed(words)));
    lab.label(1, RunsOn::Automatic);
    let (run, why) = failed_run(&lab.until_the_end());
    assert_eq!(run, 1);
    assert_eq!(
        why,
        "The model could not be run on the graphics card (Made-up card A)."
    );
    assert_eq!(lab.stand_in.opened(), [(Role::Robust1, card)]);
}

/// What a run that lost its volume has said and kept: its first two stages, then, once, that
/// the scan in view changed, for its own number; no labels, and a second network refused.
fn assert_over_because_the_scan_changed(lab: &Lab, said: &[Action]) {
    let (run, why) = failed_run(said);
    let sentence = "Labelling stopped because the scan in view changed.";
    assert_eq!((run, why.as_str()), (1, sentence));
    let Some((_, before)) = said.split_last() else {
        panic!("the run said something: {said:?}");
    };
    assert_over_part_way(lab, before);
}

#[test]
fn a_run_whose_series_gives_way_to_another_in_the_store_is_over_and_says_once_that_the_scan_changed()
 {
    let (lab, said) = held_run(Lab::another_series);
    assert_over_because_the_scan_changed(&lab, &said);
}

#[test]
fn a_run_whose_volume_is_made_again_for_its_series_is_over_and_says_once_that_the_scan_changed() {
    let (lab, said) = held_run(Lab::the_volume_made_again);
    assert_over_because_the_scan_changed(&lab, &said);
}

#[test]
fn a_run_that_was_stopped_says_nothing_when_its_volume_goes_after() {
    let (lab, said) = held_run(|lab| {
        ask(lab, Effect::StopLabelling);
        lab.another_series();
    });
    assert_over_part_way(&lab, &said);
}

#[test]
fn a_run_holds_the_turn_from_its_start_to_its_end() {
    let _alone = one_at_a_time();
    let (stand_in, running, go) = StandIn::roomy().held_while_running();
    let lab = Lab::new(stand_in);
    let held = || *stand_ins::held(&lab.workers.shared.labelling.busy);
    assert!(!held(), "nobody has it before a run");
    lab.label(1, RunsOn::Automatic);
    // Whatever the run says first, it says holding the turn.
    assert_eq!(stages(&[lab.next()], 1), [Stage::Preparing]);
    assert!(held(), "the run holds it when it says its first stage");
    running
        .recv_timeout(PATIENCE)
        .expect("the first network runs");
    assert!(held(), "and while its network is open");
    go.send(()).expect("the network waits");
    lab.until_the_end();
    lab.settled();
    assert!(!held(), "and lets it go at its end");
}

#[test]
fn a_run_asked_for_after_a_stopped_one_has_no_network_open_beside_the_stopped_ones() {
    let _alone = one_at_a_time();
    let (stand_in, running, go) = StandIn::roomy().held_while_running();
    let lab = Lab::new(stand_in);
    lab.label(1, RunsOn::Automatic);
    running
        .recv_timeout(PATIENCE)
        .expect("the first network runs");
    ask(&lab, Effect::StopLabelling);
    lab.label(2, RunsOn::Automatic);
    go.send(()).expect("the network waits");
    let said = lab.until_the_end();
    let every = [
        Stage::Preparing,
        Stage::Finding { step: 1, of: 3 },
        Stage::Finding { step: 2, of: 3 },
        Stage::Finding { step: 3, of: 3 },
        Stage::Checking,
        Stage::Tidying,
    ];
    assert_eq!(
        stages(&said, 2),
        every,
        "the new run says all of its stages"
    );
    assert_eq!(made_by_run(&said).1, Some(2));
    assert_eq!(
        lab.stand_in.most_open(),
        1,
        "never two networks open together"
    );
    let card = Device::Card(card_a().id);
    let (r1, r2, r3, scores) = (Role::Robust1, Role::Robust2, Role::Robust3, Role::Scores);
    // The stopped run's network ran to its end and its next was refused, then the new run.
    let opened = [r1, r2, r1, r2, r3, scores].map(|role| (role, card));
    assert_eq!(lab.stand_in.opened(), opened);
}
