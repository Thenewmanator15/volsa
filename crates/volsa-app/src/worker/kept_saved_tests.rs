//! What the workers do to save the labels that are open as a file of their own, and to keep
//! what the person chose for labelling. Stand-ins only (see `labelling_stand_ins.rs`).

use std::collections::BTreeSet;
use std::path::PathBuf;

use volsa_labels::read_labels;
use volsa_models::RunsOn;

use super::kept_tests::{key, notice, set_of};
use super::lab::Lab;
use super::stand_ins::{StandIn, one_at_a_time};
use crate::action::{Action, Effect};
use crate::choices::{Choices, RunsOnKept, load_choices};
use crate::kept_labels::{keep, kept};
use crate::state::NoticeKind;
use crate::testing::{made_up_labels, written_labels};

const SAVED: &str = "The labels and their table of names were saved.";
const NOT_SAVED: &str = "These labels could not be saved as a file. Only labels that Volsa \
                         made can be saved, and the folder must be one that can be written to.";

/// Labels from a run, not kept, and settled; and where to save them.
fn after_a_run() -> (Lab, PathBuf) {
    let lab = Lab::keeping(StandIn::roomy());
    lab.label_keeping(1, RunsOn::Automatic, false);
    lab.until_taken();
    let to = lab.settings().join("chosen").join("my labels.nii.gz");
    std::fs::create_dir(lab.settings().join("chosen")).expect("a folder");
    (lab, to)
}

#[test]
fn the_labels_open_are_saved_with_their_table_beside_them_and_keep_the_structures_own_numbers() {
    let _alone = one_at_a_time();
    let (lab, to) = after_a_run();
    // Saving at once, while the files are still being made ready, waits for them.
    lab.workers.run(Effect::SaveLabels { to: to.clone() }, None);
    assert_eq!(notice(lab.next()), (NoticeKind::Info, SAVED.to_owned()));
    let table = to.with_file_name("my labels.json");
    let set = set_of(&to, &table);
    assert_eq!(set, *lab.workers.labels().expect("shown"));
    let grid = read_labels(&std::fs::read(&to).expect("reads")).expect("labels");
    let numbers: BTreeSet<u32> = grid.ids.iter().copied().collect();
    assert!(
        !numbers.contains(&1),
        "the structures' numbers, not the places of the parts: {numbers:?}"
    );
    assert!(numbers.contains(&4), "{numbers:?}");
}

#[test]
fn labels_saved_under_a_name_with_no_ending_are_written_as_a_gzipped_nifti_file() {
    let _alone = one_at_a_time();
    let (lab, to) = after_a_run();
    let plain = to.with_file_name("my labels");
    lab.workers
        .run(Effect::SaveLabels { to: plain.clone() }, None);
    assert_eq!(notice(lab.next()), (NoticeKind::Info, SAVED.to_owned()));
    assert!(!plain.exists(), "nothing under a name no program reads");
    let set = set_of(&to, &to.with_file_name("my labels.json"));
    assert_eq!(set, *lab.workers.labels().expect("shown"));
}

#[test]
fn labels_shown_from_a_kept_result_are_saved_as_they_were_kept() {
    let lab = Lab::keeping(StandIn::roomy());
    let (labels, text) = written_labels();
    keep(lab.settings(), &key(), &labels, &text).expect("it is kept");
    let (from_labels, from_table) = kept(lab.settings(), &key()).expect("it is kept");
    let read = Effect::ReadLabels {
        labels: from_labels,
        table: Some(from_table),
    };
    lab.workers.run(read, None);
    assert!(matches!(lab.next(), Action::LabelsRead { run: None, .. }));
    let to = lab.settings().join("saved.nii.gz");
    lab.workers.run(Effect::SaveLabels { to: to.clone() }, None);
    assert_eq!(notice(lab.next()), (NoticeKind::Info, SAVED.to_owned()));
    assert_eq!(std::fs::read(&to).expect("reads"), labels);
    let table = std::fs::read_to_string(lab.settings().join("saved.json")).expect("reads");
    assert_eq!(table, text);
}

#[test]
fn labels_that_were_not_made_by_volsa_are_not_saved() {
    let lab = Lab::keeping(StandIn::roomy());
    let (labels, _) = written_labels();
    let (from, plain) = (
        lab.settings().join("from.nii.gz"),
        lab.settings().join("from.json"),
    );
    std::fs::write(&from, &labels).expect("writes");
    let (_, mut table) = made_up_labels();
    table.made = None;
    let text = volsa_labels::write_table(&table).expect("the table is written");
    std::fs::write(&plain, text).expect("writes");
    let read = Effect::ReadLabels {
        labels: from,
        table: Some(plain),
    };
    lab.workers.run(read, None);
    assert!(matches!(lab.next(), Action::LabelsRead { made: None, .. }));
    let to = lab.settings().join("saved.nii.gz");
    lab.workers.run(Effect::SaveLabels { to: to.clone() }, None);
    assert_eq!(
        notice(lab.next()),
        (NoticeKind::Warning, NOT_SAVED.to_owned())
    );
    assert!(!to.exists());
}

#[test]
fn labels_that_are_closed_or_replaced_are_not_the_ones_saved() {
    let lab = Lab::keeping(StandIn::roomy());
    let (labels, text) = written_labels();
    keep(lab.settings(), &key(), &labels, &text).expect("it is kept");
    let (from_labels, from_table) = kept(lab.settings(), &key()).expect("it is kept");
    let read = Effect::ReadLabels {
        labels: from_labels,
        table: Some(from_table),
    };
    lab.workers.run(read, None);
    lab.next();
    lab.workers.run(Effect::DropLabels, None);
    let to = lab.settings().join("saved.nii.gz");
    lab.workers.run(Effect::SaveLabels { to: to.clone() }, None);
    assert_eq!(
        notice(lab.next()),
        (NoticeKind::Warning, NOT_SAVED.to_owned())
    );
    assert!(!to.exists());
}

#[test]
fn labels_that_cannot_be_written_where_the_person_chose_say_so() {
    let lab = Lab::keeping(StandIn::roomy());
    let (labels, text) = written_labels();
    keep(lab.settings(), &key(), &labels, &text).expect("it is kept");
    let (from_labels, from_table) = kept(lab.settings(), &key()).expect("it is kept");
    let read = Effect::ReadLabels {
        labels: from_labels,
        table: Some(from_table),
    };
    lab.workers.run(read, None);
    lab.next();
    let to = lab.settings().join("never made").join("saved.nii.gz");
    lab.workers.run(Effect::SaveLabels { to: to.clone() }, None);
    assert_eq!(
        notice(lab.next()),
        (NoticeKind::Warning, NOT_SAVED.to_owned())
    );
    assert!(!to.exists());
}

fn unusual() -> Choices {
    Choices {
        runs_on: RunsOnKept::Card("Made-up card A".to_owned()),
        keep: false,
        ..Choices::default()
    }
}

#[test]
fn choices_are_written_to_the_settings_folder_and_the_newest_asked_for_is_the_one_left() {
    let lab = Lab::keeping(StandIn::roomy());
    for number in 0..6 {
        let older = Choices {
            cortex: number % 2 == 0,
            ..Choices::default()
        };
        lab.workers.run(Effect::StoreChoices(older), None);
    }
    lab.workers.run(Effect::StoreChoices(unusual()), None);
    Lab::eventually("the choices", || load_choices(lab.settings()) == unusual());
    // Whatever was still on its way does not take the place of the newest.
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(load_choices(lab.settings()), unusual());
}

#[test]
fn with_nowhere_to_keep_settings_choices_are_not_kept_and_nothing_is_said() {
    let lab = Lab::new(StandIn::roomy());
    lab.workers.run(Effect::StoreChoices(unusual()), None);
    assert_eq!(lab.said(), []);
}
