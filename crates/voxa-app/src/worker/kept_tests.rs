//! What the workers keep of a run: a result kept for its series, found again and deleted.
//! Stand-ins only (see `labelling_stand_ins.rs`). Saving labels and keeping what is chosen are
//! in `kept_saved_tests.rs`.

use std::path::{Path, PathBuf};

use voxa_labels::{LabelSet, label_set, read_labels, read_table};
use voxa_models::RunsOn;

use super::lab::{Lab, made_by_run};
use super::stand_ins::{PATIENCE, StandIn, WHEN, one_at_a_time};
use crate::action::{Action, Effect};
use crate::kept_labels::{keep, kept};
use crate::labelling::LabellingAction;
use crate::state::NoticeKind;
use crate::testing::{FINE_SCAN, SCOUT, THICK_SCAN, id, made_up_labels, written_labels};
use crate::views::series_key;

const NOT_KEPT: &str = "These labels could not be kept on this computer. They are shown now and \
                        will be gone when Voxa closes.";
const NOT_DELETED: &str = "The labels kept for this scan could not be deleted from this computer.";

pub(super) fn key() -> String {
    series_key(&id(THICK_SCAN))
}

/// What a notice the worker posted says, and how seriously.
pub(super) fn notice(said: Action) -> (NoticeKind, String) {
    match said {
        Action::Post(notice) => (notice.kind, notice.text),
        other => panic!("not a notice: {other:?}"),
    }
}

/// What the worker says on finding a kept result.
fn found(said: Action) -> (voxa_core::SeriesId, String, PathBuf, PathBuf) {
    match said {
        Action::Labelling(LabellingAction::KeptFound {
            series,
            when,
            labels,
            table,
        }) => (series, when, labels, table),
        other => panic!("not a find: {other:?}"),
    }
}

/// The set a pair of files makes, as opening them would.
pub(super) fn set_of(labels: &Path, table: &Path) -> LabelSet {
    let grid = read_labels(&std::fs::read(labels).expect("reads")).expect("labels");
    let text = std::fs::read_to_string(table).expect("reads");
    let named = read_table("table.json", &text).expect("a table");
    label_set(&grid, Some(&named)).expect("a set")
}

/// The names of the files kept, if a folder for them was ever made.
fn names_kept(settings: &Path) -> Vec<String> {
    let Ok(found) = std::fs::read_dir(settings.join("labels")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = found
        .map(|one| one.expect("an entry").file_name().to_string_lossy().into())
        .collect();
    names.sort();
    names
}

#[test]
fn a_result_is_kept_for_its_series_and_what_is_kept_reads_back_as_what_is_shown() {
    let _alone = one_at_a_time();
    let lab = Lab::keeping(StandIn::roomy());
    lab.label_keeping(1, RunsOn::Automatic, true);
    let said = lab.until_taken();
    let (made, run) = made_by_run(&said);
    assert_eq!(run, Some(1), "the result is reported first");
    let (series, when, labels, table) = found(lab.next());
    assert_eq!((series, when.as_str()), (id(THICK_SCAN), WHEN));
    assert_eq!(
        kept(lab.settings(), &key()),
        Some((labels.clone(), table.clone()))
    );
    assert_eq!(
        labels.parent(),
        Some(lab.settings().join("labels").as_path())
    );
    let shown = lab.workers.labels().expect("the labels are shown");
    let set = set_of(&labels, &table);
    assert_eq!(set, *shown, "the same set, opened again from its files");
    assert_eq!(set.made, Some(made), "with who made it");
    lab.settled();
    assert_eq!(lab.said(), [], "and that is all");
    assert_eq!(names_kept(lab.settings()).len(), 2, "no part file is left");
}

#[test]
fn with_keeping_off_nothing_is_written_and_what_was_kept_before_stays() {
    let _alone = one_at_a_time();
    let lab = Lab::keeping(StandIn::roomy());
    lab.label_keeping(1, RunsOn::Automatic, false);
    lab.until_taken();
    lab.settled();
    assert_eq!(lab.said(), []);
    assert_eq!(names_kept(lab.settings()), Vec::<String>::new());
    assert!(
        !lab.settings().join("labels").exists(),
        "not even the folder"
    );
    // A result kept before is left as it was.
    keep(lab.settings(), &key(), b"older labels", "older table").expect("it is kept");
    lab.label_keeping(2, RunsOn::Automatic, false);
    lab.until_taken();
    lab.settled();
    let (labels, table) = kept(lab.settings(), &key()).expect("still kept");
    assert_eq!(std::fs::read(labels).expect("reads"), b"older labels");
    assert_eq!(std::fs::read(table).expect("reads"), b"older table");
    assert_eq!(lab.said(), []);
}

#[test]
fn a_new_result_takes_the_place_of_the_one_kept_before() {
    let _alone = one_at_a_time();
    let lab = Lab::keeping(StandIn::roomy());
    keep(lab.settings(), &key(), b"older labels", "older table").expect("it is kept");
    lab.label_keeping(1, RunsOn::Automatic, true);
    lab.until_taken();
    let (_, _, labels, table) = found(lab.next());
    assert_ne!(std::fs::read(&labels).expect("reads"), b"older labels");
    assert_eq!(
        set_of(&labels, &table),
        *lab.workers.labels().expect("shown")
    );
    assert_eq!(names_kept(lab.settings()).len(), 2);
}

#[test]
fn a_result_that_cannot_be_kept_is_still_shown_and_the_warning_follows_it() {
    let _alone = one_at_a_time();
    let folder = tempfile::tempdir().expect("a temporary folder");
    let in_the_way = folder.path().join("settings");
    std::fs::write(&in_the_way, "a file where the folder should be").expect("writes");
    let lab = Lab::keeping_in(StandIn::roomy(), in_the_way);
    lab.label_keeping(1, RunsOn::Automatic, true);
    let said = lab.until_taken();
    assert!(matches!(
        said.last(),
        Some(Action::LabelsRead { run: Some(1), .. })
    ));
    assert!(lab.workers.labels().is_some(), "they are shown");
    assert_eq!(
        notice(lab.next()),
        (NoticeKind::Warning, NOT_KEPT.to_owned())
    );
    lab.settled();
    assert_eq!(lab.said(), []);
}

#[test]
fn with_nowhere_to_keep_settings_a_result_is_not_kept_and_the_warning_says_so() {
    let _alone = one_at_a_time();
    let lab = Lab::new(StandIn::roomy());
    lab.label_keeping(1, RunsOn::Automatic, true);
    lab.until_taken();
    assert_eq!(
        notice(lab.next()),
        (NoticeKind::Warning, NOT_KEPT.to_owned())
    );
}

#[test]
fn a_run_that_is_stopped_keeps_nothing() {
    let (stand_in, running, go) = StandIn::roomy().held_while_running();
    let lab = Lab::keeping(stand_in);
    lab.label_keeping(1, RunsOn::Automatic, true);
    running
        .recv_timeout(PATIENCE)
        .expect("the first network runs");
    lab.workers.run(Effect::StopLabelling, None);
    go.send(()).expect("the network waits");
    lab.settled();
    let said = lab.said();
    let kept_any =
        |said: &Action| matches!(said, Action::Labelling(LabellingAction::KeptFound { .. }));
    assert!(!said.iter().any(kept_any), "{said:?}");
    assert_eq!(names_kept(lab.settings()), Vec::<String>::new());
}

#[test]
fn nothing_kept_says_whose_scan_it_was_or_where_anything_was() {
    let _alone = one_at_a_time();
    let lab = Lab::keeping(StandIn::roomy());
    lab.label_keeping(1, RunsOn::Automatic, true);
    lab.until_taken();
    let (_, _, labels, table) = found(lab.next());
    let text = std::fs::read_to_string(&table).expect("reads");
    let bytes = std::fs::read(&labels).expect("reads");
    let places = [lab.settings(), lab.folder.path()];
    let mut forbidden = vec![THICK_SCAN.to_owned(), FINE_SCAN.to_owned()];
    for place in places {
        forbidden.push(place.to_string_lossy().into_owned());
        let last = place.file_name().expect("a name");
        forbidden.push(last.to_string_lossy().into_owned());
    }
    for word in &forbidden {
        assert!(!text.contains(word.as_str()), "the table holds {word}");
        let held = bytes
            .windows(word.len())
            .any(|part| part == word.as_bytes());
        assert!(!held, "the labels hold {word}");
    }
    // The gzip header names no file, and has no comment.
    assert_eq!(bytes.get(..2), Some(&[0x1f, 0x8b][..]));
    assert_eq!(bytes.get(3).map(|flags| flags & 0b0001_1110), Some(0));
    // The files are named by the series' key, which is a hash.
    let key = key();
    assert_eq!(
        names_kept(lab.settings()),
        [format!("{key}.json"), format!("{key}.nii.gz")]
    );
    assert!(!key.contains(THICK_SCAN));
}

#[test]
fn the_result_kept_for_a_series_is_found_with_its_day_and_its_two_files() {
    let lab = Lab::keeping(StandIn::roomy());
    let (labels, text) = written_labels();
    keep(lab.settings(), &key(), &labels, &text).expect("it is kept");
    lab.workers.run(
        Effect::FindKept {
            series: id(THICK_SCAN),
        },
        None,
    );
    let (series, when, labels, table) = found(lab.next());
    let (_, made) = made_up_labels();
    let day = made.made.expect("said when").when;
    assert_eq!((series, when), (id(THICK_SCAN), day));
    assert_eq!(kept(lab.settings(), &key()), Some((labels, table)));
}

#[test]
fn nothing_is_said_of_a_series_with_no_result_kept_or_one_whose_table_is_damaged() {
    let lab = Lab::keeping(StandIn::roomy());
    let (labels, text) = written_labels();
    // The fine series has none; the scout has labels and a table that is not one.
    let scout = series_key(&id(SCOUT));
    keep(lab.settings(), &scout, &labels, "this is not a table").expect("it is kept");
    keep(lab.settings(), &key(), &labels, &text).expect("it is kept");
    for series in [FINE_SCAN, SCOUT, THICK_SCAN] {
        lab.workers
            .run(Effect::FindKept { series: id(series) }, None);
    }
    let (series, ..) = found(lab.next());
    assert_eq!(series, id(THICK_SCAN), "only the whole result is found");
    assert_eq!(lab.said(), []);
}

#[test]
fn with_nowhere_to_keep_settings_nothing_is_found() {
    let lab = Lab::new(StandIn::roomy());
    lab.workers.run(
        Effect::FindKept {
            series: id(THICK_SCAN),
        },
        None,
    );
    lab.workers.run(
        Effect::ForgetKept {
            series: id(THICK_SCAN),
        },
        None,
    );
    assert_eq!(lab.said(), []);
}

#[test]
fn a_result_that_is_deleted_is_gone_and_a_series_the_others_are_kept_for_is_left_alone() {
    let lab = Lab::keeping(StandIn::roomy());
    keep(lab.settings(), &key(), b"labels", "table").expect("it is kept");
    let other = series_key(&id(FINE_SCAN));
    keep(lab.settings(), &other, b"other labels", "other table").expect("it is kept");
    lab.workers.run(
        Effect::ForgetKept {
            series: id(THICK_SCAN),
        },
        None,
    );
    Lab::eventually("the result to go", || {
        kept(lab.settings(), &key()).is_none()
    });
    Lab::eventually("the rest to go", || names_kept(lab.settings()).len() == 2);
    assert!(kept(lab.settings(), &other).is_some());
}

#[test]
fn a_result_that_cannot_be_deleted_says_so() {
    let lab = Lab::keeping(StandIn::roomy());
    keep(lab.settings(), &key(), b"labels", "table").expect("it is kept");
    // A folder where the label file should be: it cannot be deleted as a file.
    let labels = lab
        .settings()
        .join("labels")
        .join(format!("{}.nii.gz", key()));
    std::fs::remove_file(&labels).expect("removes");
    std::fs::create_dir(&labels).expect("a folder in the way");
    lab.workers.run(
        Effect::ForgetKept {
            series: id(THICK_SCAN),
        },
        None,
    );
    assert_eq!(
        notice(lab.next()),
        (NoticeKind::Warning, NOT_DELETED.to_owned())
    );
}
