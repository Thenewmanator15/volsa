//! Headers read on several threads give the index, and the progress, that reading them in
//! turn gives.

use super::reading_tests::{folder_of, made_up_study, zip_of};
use super::tests::stack;
use super::*;
use crate::source::open_source;
use crate::testkit::{MemorySource, SliceOptions};

/// The made-up study and three made-up series of 100 slices each: over 300 files.
fn three_hundred_files() -> Vec<(String, Vec<u8>)> {
    let mut files = made_up_study();
    for number in [21, 22, 23] {
        let base = SliceOptions {
            series_uid: format!("1.2.826.0.1.3680043.9.9999.2.{number}"),
            series_number: number,
            ..SliceOptions::default()
        };
        files.extend(stack(100, 2.0, &base));
    }
    files
}

#[test]
fn an_index_read_on_several_threads_is_the_same_index() {
    let files = three_hundred_files();
    let mut in_turn = Vec::new();
    let read_in_turn = index(&mut MemorySource::new(files.clone()), &mut |found| {
        in_turn.push(found.clone());
    })
    .expect("indexes");
    let folder = tempfile::tempdir().expect("a temporary folder");
    folder_of(&folder.path().join("export"), &files);
    let stored = folder.path().join("stored.zip");
    zip_of(&stored, &files, zip::CompressionMethod::Stored);
    let deflated = folder.path().join("deflated.zip");
    zip_of(&deflated, &files, zip::CompressionMethod::Deflated);
    for path in [folder.path().join("export"), stored, deflated] {
        let mut source = open_source(&path).expect("opens");
        assert!(source.names().is_some(), "{path:?} can say its names");
        let mut reports = Vec::new();
        let read = index(source.as_mut(), &mut |found| reports.push(found.clone()));
        assert_eq!(read.expect("indexes"), read_in_turn, "{path:?}");
        assert_eq!(reports, in_turn, "{path:?}: progress as before");
    }
}

#[test]
fn a_file_that_cannot_be_read_stops_the_index_as_before() {
    let files = three_hundred_files();
    let folder = tempfile::tempdir().expect("a temporary folder");
    folder_of(folder.path(), &files);
    let mut source = open_source(folder.path()).expect("opens");
    let names = source.names().expect("a folder says its names");
    let gone = names[names.len() / 2].clone();
    std::fs::remove_file(folder.path().join(&gone)).expect("removed");
    let mut last = None;
    let read = index(source.as_mut(), &mut |found| last = Some(found.clone()));
    assert!(matches!(read, Err(VoxaError::Io(_))), "{read:?}");
    let last = last.expect("progress was reported before the missing file");
    let added = last.series.iter().flat_map(|series| &series.slices);
    let later = added.filter(|slice| slice.entry > gone).count();
    assert_eq!(later, 0, "no file after the missing one is added");
}
