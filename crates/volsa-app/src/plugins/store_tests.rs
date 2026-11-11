//! Volsa's own folder of plugins: a copy added only whole, named by its sum, found again, and
//! removed.

use std::fs;

use volsa_models::SUPPORTED;
use volsa_plugins::testkit::example_folder;
use volsa_plugins::{Offered, check_folder};

use super::store::{add, list, remove, same_id};

/// The example plugin of a version, offered from a folder that also holds a file it does not
/// list.
fn offered(version: &str) -> (tempfile::TempDir, Offered) {
    let source = example_folder(version);
    fs::write(source.path().join("notes.txt"), b"not listed").expect("written");
    let offered = check_folder(source.path(), SUPPORTED).expect("offered");
    (source, offered)
}

#[test]
fn adding_copies_only_the_listed_files_into_a_folder_named_by_the_sum() {
    let plugins = tempfile::tempdir().expect("a folder");
    let (_source, offered) = offered("1.0");
    let installed = add(&offered, plugins.path(), SUPPORTED).expect("added");
    assert_eq!(installed.folder, plugins.path().join(&offered.sum[..16]));
    let mut names: Vec<String> = fs::read_dir(&installed.folder)
        .expect("listed")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(names, ["plugin.json", "strip.onnx"]);
    assert_eq!(installed.description.version, "1.0");
}

#[test]
fn a_failed_copy_leaves_no_folder_behind() {
    let plugins = tempfile::tempdir().expect("a folder");
    let (source, offered) = offered("1.0");
    fs::remove_file(source.path().join("strip.onnx")).expect("removed");
    assert!(add(&offered, plugins.path(), SUPPORTED).is_err());
    let left = fs::read_dir(plugins.path()).expect("listed").count();
    assert_eq!(left, 0);
}

#[test]
fn a_plugin_with_the_same_id_is_found_for_replacing() {
    let plugins = tempfile::tempdir().expect("a folder");
    let (_first_source, first) = offered("1.0");
    let installed = add(&first, plugins.path(), SUPPORTED).expect("added");
    let (_second_source, second) = offered("2.0");
    let all = [installed];
    let found = same_id(&all, &second).expect("the same id");
    assert_eq!(found.description.version, "1.0");
}

#[test]
fn listing_finds_each_plugin_and_says_why_one_is_broken() {
    let plugins = tempfile::tempdir().expect("a folder");
    let (_source, offered) = offered("1.0");
    add(&offered, plugins.path(), SUPPORTED).expect("added");
    fs::create_dir(plugins.path().join("0123456789abcdef")).expect("a broken copy");
    let listed = list(plugins.path(), SUPPORTED);
    assert_eq!(listed.len(), 2);
    assert_eq!(listed.iter().filter(|plugin| plugin.is_ok()).count(), 1);
}

#[test]
fn removing_deletes_the_copy() {
    let plugins = tempfile::tempdir().expect("a folder");
    let (_source, offered) = offered("1.0");
    let installed = add(&offered, plugins.path(), SUPPORTED).expect("added");
    remove(&installed.folder).expect("removed");
    assert!(!installed.folder.exists());
    assert_eq!(list(plugins.path(), SUPPORTED).len(), 0);
}
