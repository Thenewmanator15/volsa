//! What Voxa 0.5.0 and earlier kept of scans in the roaming settings folder is moved to the
//! folder on this computer alone, once, whole, and never twice.

use std::path::Path;

use tempfile::TempDir;

use super::*;

/// Write a file, making its folder.
fn put(folder: &Path, name: &str, bytes: &[u8]) {
    let file = folder.join(name);
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).expect("a folder");
    }
    std::fs::write(file, bytes).expect("writes");
}

/// The bytes of a file, or `None` when there is none.
fn bytes(folder: &Path, name: &str) -> Option<Vec<u8>> {
    std::fs::read(folder.join(name)).ok()
}

/// Every file under a folder, by its path within it, with forward slashes, in order.
fn files_in(folder: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(folder) else {
        return found;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() {
            found.extend(
                files_in(&entry.path())
                    .into_iter()
                    .map(|inner| format!("{name}/{inner}")),
            );
        } else {
            found.push(name);
        }
    }
    found.sort();
    found
}

/// A roaming settings folder as Voxa 0.5.0 left it: the preferences, the saved views, the
/// annotations, the choices for labelling, two kept results, and a file of views half
/// written when Voxa stopped.
fn as_0_5_0_left_it() -> TempDir {
    let roaming = tempfile::tempdir().expect("a temporary folder");
    let files: [(&str, &[u8]); 9] = [
        ("preferences.json", b"{\"viewports\": 4}"),
        ("views.json", b"{\"0123456789abcdef\": []}"),
        ("views.json.new", b"{\"0123"),
        ("annotations.json", b"{\"version\": 1, \"series\": {}}"),
        ("choices.json", b"{\"version\": 1}"),
        ("labels/0123456789abcdef.nii.gz", b"made-up labels"),
        ("labels/0123456789abcdef.json", b"made-up table"),
        ("labels/fedcba9876543210.nii.gz", b"other made-up labels"),
        ("labels/fedcba9876543210.json", b"other made-up table"),
    ];
    for (name, contents) in files {
        put(roaming.path(), name, contents);
    }
    roaming
}

#[test]
fn what_is_kept_of_scans_is_moved_whole_and_the_preferences_stay() {
    let roaming = as_0_5_0_left_it();
    let before: Vec<(String, Vec<u8>)> = files_in(roaming.path())
        .into_iter()
        .map(|name| {
            let held = bytes(roaming.path(), &name).expect("there");
            (name, held)
        })
        .collect();
    let local = tempfile::tempdir().expect("a temporary folder");
    let here = local.path().join("data");
    let moved = move_kept(roaming.path(), &here);
    assert_eq!(files_in(roaming.path()), ["preferences.json"]);
    assert!(
        !roaming.path().join("labels").exists(),
        "its folder goes too"
    );
    for (name, held) in before.iter().filter(|(name, _)| name != "preferences.json") {
        assert_eq!(bytes(&here, name).as_ref(), Some(held), "{name}");
    }
    assert_eq!(files_in(&here).len(), 8, "{:?}", files_in(&here));
    assert_eq!(
        moved.sentence().as_deref(),
        Some(
            "Moved Voxa's kept files out of the roaming settings folder into the local one: \
             annotations.json, choices.json, views.json, views.json.new and 4 files of \
             kept labelling results."
        )
    );
}

#[test]
fn a_second_start_finds_nothing_to_move_and_says_nothing() {
    let roaming = as_0_5_0_left_it();
    let local = tempfile::tempdir().expect("a temporary folder");
    assert!(move_kept(roaming.path(), local.path()).sentence().is_some());
    let again = move_kept(roaming.path(), local.path());
    assert_eq!(again, Moved::default());
    assert_eq!(again.sentence(), None);
    let nowhere = local.path().join("never made");
    assert_eq!(move_kept(&nowhere, local.path()), Moved::default());
}

#[test]
fn a_different_file_already_in_the_local_folder_is_kept_and_the_old_one_left_where_it_was() {
    let roaming = as_0_5_0_left_it();
    let local = tempfile::tempdir().expect("a temporary folder");
    put(local.path(), "views.json", b"{\"newer\": []}");
    put(
        local.path(),
        "labels/0123456789abcdef.json",
        b"a newer table",
    );
    let moved = move_kept(roaming.path(), local.path());
    let views = bytes(local.path(), "views.json");
    assert_eq!(views.as_deref(), Some(&b"{\"newer\": []}"[..]));
    let left = bytes(roaming.path(), "views.json");
    assert_eq!(left.as_deref(), Some(&b"{\"0123456789abcdef\": []}"[..]));
    // A result is two files, which stay together: both are left, not only the one in the way.
    for name in ["0123456789abcdef.json", "0123456789abcdef.nii.gz"] {
        let old = bytes(&roaming.path().join("labels"), name);
        assert!(old.is_some(), "{name} is left");
    }
    let table = bytes(local.path(), "labels/0123456789abcdef.json");
    assert_eq!(table.as_deref(), Some(&b"a newer table"[..]));
    assert!(!local.path().join("labels/0123456789abcdef.nii.gz").exists());
    assert!(local.path().join("labels/fedcba9876543210.nii.gz").exists());
    assert_eq!(
        moved.sentence().as_deref(),
        Some(
            "Moved Voxa's kept files out of the roaming settings folder into the local one: \
             annotations.json, choices.json, views.json.new and 2 files of kept labelling \
             results. Left views.json and 2 files of kept labelling results in the roaming \
             settings folder: the local one has a different file of the same name, or the move \
             failed."
        )
    );
}

#[test]
fn the_same_file_on_both_sides_is_a_move_cut_short_and_the_old_copy_goes() {
    let roaming = as_0_5_0_left_it();
    let local = tempfile::tempdir().expect("a temporary folder");
    put(local.path(), "choices.json", b"{\"version\": 1}");
    let moved = move_kept(roaming.path(), local.path());
    assert_eq!(files_in(roaming.path()), ["preferences.json"]);
    assert!(
        moved.files.contains(&"choices.json".to_owned()),
        "{moved:?}"
    );
}

#[test]
fn one_folder_given_twice_moves_nothing_and_loses_nothing() {
    let roaming = as_0_5_0_left_it();
    let before = files_in(roaming.path());
    assert_eq!(move_kept(roaming.path(), roaming.path()), Moved::default());
    let same = roaming.path().join("labels").join("..");
    assert_eq!(move_kept(roaming.path(), &same), Moved::default());
    assert_eq!(files_in(roaming.path()), before);
}

#[test]
fn a_local_folder_that_cannot_be_made_leaves_everything_where_it_was() {
    let roaming = as_0_5_0_left_it();
    let before = files_in(roaming.path());
    let local = tempfile::tempdir().expect("a temporary folder");
    let in_the_way = local.path().join("a file");
    std::fs::write(&in_the_way, "not a folder").expect("writes");
    let moved = move_kept(roaming.path(), &in_the_way);
    assert_eq!(files_in(roaming.path()), before);
    assert!(moved.files.is_empty() && moved.results == 0, "{moved:?}");
    let said = moved.sentence().expect("something is said");
    assert!(
        said.starts_with(
            "Left annotations.json, choices.json, views.json, views.json.new and 4 files"
        ),
        "{said}"
    );
}

#[test]
fn a_file_copied_to_another_drive_is_put_in_place_before_the_old_one_goes() {
    let (from, to) = (
        tempfile::tempdir().expect("a temporary folder"),
        tempfile::tempdir().expect("a temporary folder"),
    );
    put(from.path(), "views.json", b"views");
    let (old, new) = (from.path().join("views.json"), to.path().join("views.json"));
    assert_eq!(copy_across(&old, &new), Outcome::Moved);
    assert_eq!(
        bytes(to.path(), "views.json").as_deref(),
        Some(&b"views"[..])
    );
    assert!(!old.exists());
    assert_eq!(
        files_in(to.path()),
        ["views.json"],
        "nothing is left beside it"
    );
    // Where the copy cannot be made, the file stays where it was and nothing is left behind.
    put(from.path(), "views.json", b"views");
    let nowhere = to.path().join("views.json").join("inside a file");
    assert_eq!(copy_across(&old, &nowhere), Outcome::Left);
    assert_eq!(
        bytes(from.path(), "views.json").as_deref(),
        Some(&b"views"[..])
    );
    assert_eq!(files_in(to.path()), ["views.json"]);
}

#[test]
fn the_sentence_names_only_voxas_own_files_and_counts_the_rest() {
    let moved = Moved {
        files: vec!["views.json".to_owned()],
        results: 1,
        left: Vec::new(),
        left_results: 0,
    };
    assert_eq!(
        moved.sentence().as_deref(),
        Some(
            "Moved Voxa's kept files out of the roaming settings folder into the local one: \
             views.json and 1 file of kept labelling results."
        )
    );
    let only_results = Moved {
        results: 2,
        ..Moved::default()
    };
    assert_eq!(
        only_results.sentence().as_deref(),
        Some(
            "Moved Voxa's kept files out of the roaming settings folder into the local one: \
             2 files of kept labelling results."
        )
    );
}
