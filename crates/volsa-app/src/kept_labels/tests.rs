//! A result kept for its series: kept whole or not at all, found, read back, replaced and
//! deleted. Made-up labels only, in temporary folders.

use std::path::Path;

use tempfile::TempDir;
use voxa_core::SeriesId;
use voxa_labels::{label_set, read_labels, read_table};

use super::*;
use crate::testing::{made_up_labels, written_labels as written};
use crate::views::series_key;

/// A made-up scan's identifier.
const SCAN: &str = "1.2.3.made.up.scan";

fn key() -> String {
    series_key(&SeriesId(SCAN.to_owned()))
}

fn folder() -> TempDir {
    tempfile::tempdir().expect("a temporary folder")
}

/// The names of the files in the folder the results are kept in.
fn names_in(folder: &Path) -> Vec<String> {
    let found = std::fs::read_dir(folder.join("labels")).expect("the folder is there");
    let mut names: Vec<String> = found
        .map(|one| one.expect("an entry").file_name().to_string_lossy().into())
        .collect();
    names.sort();
    names
}

#[test]
fn a_result_that_is_kept_is_found_and_reads_back_as_the_set_with_who_made_it() {
    let folder = folder();
    let (labels, text) = written();
    keep(folder.path(), &key(), &labels, &text).expect("it is kept");
    let (labels_file, table_file) = kept(folder.path(), &key()).expect("it is found");
    let read_back = read_labels(&std::fs::read(labels_file).expect("reads")).expect("labels");
    let table_text = std::fs::read_to_string(table_file).expect("reads");
    let named = read_table("table.json", &table_text).expect("a table");
    let set = label_set(&read_back, Some(&named)).expect("a set");
    let (grid, table) = made_up_labels();
    let original = label_set(&grid, Some(&table)).expect("a set");
    assert_eq!(set, original);
    assert_eq!(set.made, table.made, "and who made it, and when");
}

#[test]
fn a_result_is_named_by_the_key_of_its_series_and_the_names_say_nothing_of_the_scan() {
    let folder = folder();
    let (labels, text) = written();
    keep(folder.path(), &key(), &labels, &text).expect("it is kept");
    let key = key();
    assert_eq!(
        names_in(folder.path()),
        [format!("{key}.json"), format!("{key}.nii.gz")]
    );
    assert!(!key.contains("made") && !key.contains("1.2.3"), "{key}");
    let (labels_file, table_file) = kept(folder.path(), &key).expect("it is found");
    for file in [labels_file, table_file] {
        let whole = file.to_string_lossy();
        assert!(!whole.contains(SCAN), "{whole}");
    }
}

#[test]
fn files_still_under_their_part_names_are_not_a_kept_result() {
    let folder = folder();
    let (labels, text) = written();
    keep(folder.path(), &key(), &labels, &text).expect("it is kept");
    let key = key();
    let inside = folder.path().join("labels");
    for end in ["nii.gz", "json"] {
        let whole = inside.join(format!("{key}.{end}"));
        std::fs::rename(&whole, inside.join(format!("{key}.{end}.part"))).expect("renames");
    }
    assert_eq!(kept(folder.path(), &key), None);
}

#[test]
fn labels_without_their_table_are_not_a_kept_result_and_nor_is_a_table_without_labels() {
    let folder = folder();
    let (labels, text) = written();
    keep(folder.path(), &key(), &labels, &text).expect("it is kept");
    let key = key();
    let inside = folder.path().join("labels");
    std::fs::remove_file(inside.join(format!("{key}.json"))).expect("removes");
    assert_eq!(kept(folder.path(), &key), None, "no table");
    std::fs::write(inside.join(format!("{key}.json")), &text).expect("writes");
    std::fs::remove_file(inside.join(format!("{key}.nii.gz"))).expect("removes");
    assert_eq!(kept(folder.path(), &key), None, "no labels");
}

#[test]
fn a_file_with_nothing_in_it_is_not_whole() {
    let folder = folder();
    let (_, text) = written();
    keep(folder.path(), &key(), b"", &text).expect("it is kept");
    assert_eq!(kept(folder.path(), &key()), None);
}

#[test]
fn nothing_is_found_for_a_series_nothing_was_kept_for_or_in_a_folder_that_is_not_there() {
    let folder = folder();
    let (labels, text) = written();
    keep(folder.path(), "someoneelse", &labels, &text).expect("it is kept");
    assert_eq!(kept(folder.path(), &key()), None);
    assert_eq!(kept(&folder.path().join("never made"), &key()), None);
}

#[test]
fn keeping_again_replaces_the_result_and_leaves_no_part_file() {
    let folder = folder();
    let (labels, text) = written();
    keep(folder.path(), &key(), &labels, &text).expect("it is kept");
    keep(folder.path(), &key(), b"newer labels", "newer table").expect("and again");
    let (labels_file, table_file) = kept(folder.path(), &key()).expect("it is found");
    assert_eq!(std::fs::read(labels_file).expect("reads"), b"newer labels");
    assert_eq!(std::fs::read(table_file).expect("reads"), b"newer table");
    assert_eq!(names_in(folder.path()).len(), 2, "and nothing else");
}

#[test]
fn the_results_of_two_series_do_not_disturb_each_other() {
    let folder = folder();
    keep(folder.path(), "one", b"labels of one", "table of one").expect("it is kept");
    keep(folder.path(), "two", b"labels of two", "table of two").expect("it is kept");
    forget(folder.path(), "one").expect("it is forgotten");
    assert_eq!(kept(folder.path(), "one"), None);
    assert!(kept(folder.path(), "two").is_some());
}

#[test]
fn forgetting_takes_away_both_files_and_any_left_being_written() {
    let folder = folder();
    let (labels, text) = written();
    keep(folder.path(), &key(), &labels, &text).expect("it is kept");
    let key = key();
    let left = folder
        .path()
        .join("labels")
        .join(format!("{key}.json.part"));
    std::fs::write(&left, "half a table").expect("writes");
    forget(folder.path(), &key).expect("it is forgotten");
    assert_eq!(kept(folder.path(), &key), None);
    assert_eq!(names_in(folder.path()), Vec::<String>::new());
    forget(folder.path(), &key).expect("what is not there is as good as forgotten");
    forget(&folder.path().join("never made"), &key).expect("and so is a folder not there");
}

#[test]
fn a_key_that_could_name_a_place_outside_the_folder_is_refused() {
    let folder = folder();
    let inside = folder.path().join("settings");
    for key in ["", "..", "../out", "a/b", "a\\b", "a.b", "a b"] {
        let refused = keep(&inside, key, b"labels", "table").expect_err(key);
        assert_eq!(refused.kind(), std::io::ErrorKind::InvalidInput, "{key}");
        assert_eq!(kept(&inside, key), None, "{key}");
        assert!(forget(&inside, key).is_err(), "{key}");
    }
    assert!(!inside.exists(), "nothing was made");
    assert_eq!(std::fs::read_dir(folder.path()).expect("reads").count(), 0);
}

#[test]
fn a_folder_that_cannot_be_written_is_an_error_and_leaves_nothing_behind() {
    let folder = folder();
    let in_the_way = folder.path().join("settings");
    std::fs::write(&in_the_way, "a file where the folder should be").expect("writes");
    assert!(keep(&in_the_way, &key(), b"labels", "table").is_err());
    assert_eq!(kept(&in_the_way, &key()), None);
    assert_eq!(std::fs::read_dir(folder.path()).expect("reads").count(), 1);
}

#[test]
fn a_failure_part_way_takes_back_the_files_it_was_writing() {
    let folder = folder();
    keep(folder.path(), &key(), b"old labels", "old table").expect("it is kept");
    let key = key();
    // A folder where the table goes cannot be taken away as the older table is, so the keeping
    // fails after both new files have been written under their part names.
    let table_file = folder.path().join("labels").join(format!("{key}.json"));
    std::fs::remove_file(&table_file).expect("removes");
    std::fs::create_dir(&table_file).expect("a folder in the way");
    assert!(keep(folder.path(), &key, b"new labels", "new table").is_err());
    let names = names_in(folder.path());
    let part = |name: &String| Path::new(name).extension().is_some_and(|end| end == "part");
    assert!(!names.iter().any(part), "{names:?}");
}

#[test]
fn a_whole_file_is_written_beside_itself_and_moved_into_place() {
    let folder = folder();
    let file = folder.path().join("saved.nii.gz");
    write_whole(&file, b"first").expect("writes");
    write_whole(&file, b"second").expect("writes over it");
    assert_eq!(std::fs::read(&file).expect("reads"), b"second");
    let count = std::fs::read_dir(folder.path()).expect("reads").count();
    assert_eq!(count, 1, "no part file is left");
    assert!(write_whole(&folder.path().join("never made").join("x"), b"x").is_err());
}

#[test]
fn the_table_of_saved_labels_goes_beside_them_under_the_same_name_with_json() {
    let table_for = |name: &str| table_for(&Path::new("somewhere").join(name));
    let beside = |name: &str| Path::new("somewhere").join(name);
    assert_eq!(table_for("head.nii.gz"), beside("head.json"));
    assert_eq!(table_for("head.nii"), beside("head.json"));
    assert_eq!(table_for("Head.NII.GZ"), beside("Head.json"));
    assert_eq!(table_for("labels.2.nii.gz"), beside("labels.2.json"));
    assert_eq!(table_for("head"), beside("head.json"));
}

#[test]
fn saved_labels_are_written_under_a_name_that_ends_as_a_gzipped_nifti_file_does() {
    let saved_as = |name: &str| saved_as(&Path::new("somewhere").join(name));
    let named = |name: &str| Path::new("somewhere").join(name);
    assert_eq!(saved_as("head.nii.gz"), named("head.nii.gz"));
    assert_eq!(saved_as("Head.NII.GZ"), named("Head.NII.GZ"));
    assert_eq!(saved_as("head.nii"), named("head.nii.gz"));
    assert_eq!(saved_as("head.gz"), named("head.nii.gz"));
    assert_eq!(saved_as("head"), named("head.nii.gz"));
    assert_eq!(saved_as("labels.2"), named("labels.2.nii.gz"));
    // And the table beside it is found again where Voxa looks for it.
    assert_eq!(table_for(&saved_as("head.gz")), named("head.json"));
}
