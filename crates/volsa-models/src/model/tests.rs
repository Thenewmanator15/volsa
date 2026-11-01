use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::{Entry, Model, ModelError, ModelSide, Role, check_file, hex, read_model, sum_of};
use crate::testkit::{FolderOptions, model_folder};

/// Words that would show a Rust type name, a path or a file name in a sentence.
const NOT_IN_A_SENTENCE: &str = "ModelError NotAModel NotFound WrongSize WrongSum Missing Lists \
                                 Format( Role :: / \\ .onnx .json .tmp";

/// The sum of these bytes, written out by the hashing crate itself.
fn expected_sum(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Checks that an error reads as a plain sentence: no type name, no path, no file name.
fn plain(error: &ModelError, folder: &Path) {
    let sentence = error.to_string();
    assert!(sentence.ends_with('.'), "not a sentence: {sentence}");
    for word in NOT_IN_A_SENTENCE.split_whitespace() {
        assert!(!sentence.contains(word), "`{word}` in: {sentence}");
    }
    let folder = folder.to_string_lossy();
    assert!(!sentence.contains(&*folder), "the folder in: {sentence}");
}

/// Reads a folder made with these changes to the default and returns what went wrong.
fn failure(change: impl FnOnce(&mut FolderOptions)) -> ModelError {
    let mut options = FolderOptions::default();
    change(&mut options);
    let folder = model_folder(&options);
    let error = read_model(folder.path()).expect_err("the folder should not read");
    plain(&error, folder.path());
    error
}

/// Reads a default folder after something was done to it on disk and returns what went wrong.
fn damaged(damage: impl FnOnce(&Path)) -> ModelError {
    let folder = model_folder(&FolderOptions::default());
    damage(folder.path());
    let error = read_model(folder.path()).expect_err("the folder should not read");
    plain(&error, folder.path());
    error
}

/// The reason a list rule gave for a folder made with these changes to the default.
fn why(change: impl FnOnce(&mut FolderOptions)) -> &'static str {
    match failure(change) {
        ModelError::Lists(why) => why,
        other => panic!("expected a broken rule, got {other:?}"),
    }
}

fn default_model() -> (TempDir, Model) {
    let folder = model_folder(&FolderOptions::default());
    let model = read_model(folder.path()).expect("the default folder should read");
    (folder, model)
}

/// Whether a folder made with these changes to the default reads.
fn reads(change: impl FnOnce(&mut FolderOptions)) -> bool {
    let mut options = FolderOptions::default();
    change(&mut options);
    read_model(model_folder(&options).path()).is_ok()
}

#[test]
fn the_default_folder_reads() {
    let (folder, model) = default_model();
    let lists = &model.lists;
    assert_eq!(lists.structures.len(), 33);
    assert_eq!(lists.areas.len(), 69);
    assert_eq!((lists.flip.len(), lists.score_class.len()), (33, 33));
    assert_eq!((lists.stage_classes, lists.box_side), (5, 224));
    assert_eq!(lists.structures.first().map(|s| s.label), Some(0));
    assert_eq!(
        lists.structures.get(1).map(|s| s.side),
        Some(ModelSide::Left)
    );
    assert_eq!(lists.areas.first().map(|a| a.class), Some(0));
    assert!(model.file(Role::Cortex).ends_with("cortex.onnx"));
    assert!(model.file(Role::Cortex).starts_with(folder.path()));
}

#[test]
fn the_six_roles_have_six_different_files_that_exist() {
    let (_folder, model) = default_model();
    let files: HashSet<_> = Role::ALL.iter().map(|&role| model.file(role)).collect();
    assert_eq!(files.len(), 6);
    assert!(files.iter().all(|file| file.is_file()));
}

#[test]
fn the_sum_is_that_of_the_list_and_changes_with_it() {
    let (folder, model) = default_model();
    assert_eq!(model.sum.len(), 64);
    assert!(
        model
            .sum
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    let list = fs::read(folder.path().join("model.json")).expect("the list is there");
    assert_eq!(model.sum, expected_sum(&list));

    let other = FolderOptions {
        stage_classes: 6,
        ..FolderOptions::default()
    };
    let other_folder = model_folder(&other);
    let other_model = read_model(other_folder.path()).expect("this folder should read too");
    assert_ne!(model.sum, other_model.sum);
}

#[test]
fn no_folder_is_not_found_and_neither_is_a_file() {
    let folder = model_folder(&FolderOptions::default());
    for place in ["nowhere", "model.json"] {
        let place = folder.path().join(place);
        let error = read_model(&place).expect_err("this is not a folder");
        assert_eq!(error, ModelError::NotFound);
        plain(&error, &place);
    }
}

#[test]
fn a_folder_without_a_list_is_not_a_model() {
    let error = damaged(|folder| fs::remove_file(folder.join("model.json")).expect("removed"));
    assert_eq!(error, ModelError::NotAModel);
}

#[test]
fn a_list_that_is_not_json_is_not_a_model() {
    let error = damaged(|folder| fs::write(folder.join("model.json"), "{").expect("written"));
    assert_eq!(error, ModelError::NotAModel);
}

/// Reads the default folder after its list was padded with spaces (so still JSON) to this size.
fn read_with_list_padded_to(size: usize) -> Result<Model, ModelError> {
    let folder = model_folder(&FolderOptions::default());
    let path = folder.path().join("model.json");
    let mut text = fs::read(&path).expect("read");
    text.resize(size, b' ');
    fs::write(path, text).expect("written");
    read_model(folder.path())
}

#[test]
fn a_list_of_one_mebibyte_reads_but_one_of_two_is_not_a_model() {
    assert!(read_with_list_padded_to(1 << 20).is_ok());
    for size in [(1 << 20) + 1, 2 << 20] {
        let error = read_with_list_padded_to(size).expect_err("too long");
        assert_eq!(error, ModelError::NotAModel, "{size} bytes");
    }
}

#[test]
fn a_list_with_parts_missing_is_not_a_model() {
    let error = damaged(|folder| {
        let text = r#"{"format":1,"kind":"synthseg-2.0"}"#;
        fs::write(folder.join("model.json"), text).expect("written");
    });
    assert_eq!(error, ModelError::NotAModel);
}

#[test]
fn another_format_is_said_with_its_number() {
    assert_eq!(failure(|o| o.format = 2), ModelError::Format(2));
}

#[test]
fn the_format_is_looked_at_before_the_rest_of_the_list() {
    let error = damaged(|folder| {
        let text = r#"{"format":7,"files":"in a shape not known yet"}"#;
        fs::write(folder.join("model.json"), text).expect("written");
    });
    assert_eq!(error, ModelError::Format(7));
}

#[test]
fn another_kind_is_said() {
    assert_eq!(failure(|o| o.kind = "other".to_owned()), ModelError::Kind);
}

#[test]
fn each_network_is_looked_for_and_checked_for_size_and_sum() {
    for role in Role::ALL {
        let (folder, model) = default_model();
        let path = model.file(role).to_path_buf();
        let original = fs::read(&path).expect("read");
        let read = || read_model(folder.path()).err();

        fs::remove_file(&path).expect("removed");
        assert_eq!(read(), Some(ModelError::Missing(role)));

        let mut longer = original.clone();
        longer.push(0);
        fs::write(&path, longer).expect("written");
        assert_eq!(read(), Some(ModelError::WrongSize(role)));

        fs::write(&path, original.get(1..).unwrap_or_default()).expect("written");
        assert_eq!(read(), Some(ModelError::WrongSize(role)));

        let mut changed = original.clone();
        changed[0] ^= 1; // one byte, same length
        fs::write(&path, changed).expect("written");
        assert_eq!(read(), Some(ModelError::WrongSum(role)));

        fs::write(&path, original).expect("written");
        assert!(read().is_none());
    }
}

#[test]
fn a_folder_in_place_of_a_network_is_missing() {
    let error = damaged(|folder| {
        fs::remove_file(folder.join("scores.onnx")).expect("removed");
        fs::create_dir(folder.join("scores.onnx")).expect("made");
    });
    assert_eq!(error, ModelError::Missing(Role::Scores));
}

#[test]
fn the_errors_name_the_network_in_plain_words() {
    let said = |role| ModelError::WrongSum(role).to_string();
    assert!(said(Role::Robust2).contains("robust 2"));
    assert_ne!(said(Role::Robust1), said(Role::Robust3));
}

#[test]
fn a_network_is_held_to_its_limit_and_its_sum_is_read_without_regard_to_capitals() {
    let (_folder, model) = default_model();
    let path = model.file(Role::Cortex);
    let bytes = fs::read(path).expect("read");
    let size = u64::try_from(bytes.len()).expect("a size");
    let capitals = Entry {
        name: "cortex.onnx".to_owned(),
        bytes: size,
        sha256: expected_sum(&bytes).to_uppercase(),
    };
    assert_eq!(check_file(path, &capitals, size, Role::Cortex), Ok(()));
    let error = Err(ModelError::WrongSize(Role::Cortex));
    assert_eq!(check_file(path, &capitals, size - 1, Role::Cortex), error);
}

#[test]
fn names_that_are_not_plain_file_names_are_not_a_model() {
    for name in [
        "../x.onnx",
        "a/b.onnx",
        "a\\b.onnx",
        "..",
        ".",
        "",
        "c:x.onnx",
    ] {
        let error = failure(|o| drop(o.names.insert(Role::Scores, name.to_owned())));
        assert_eq!(error, ModelError::NotAModel, "name {name:?}");
    }
}

#[test]
fn a_plain_name_other_than_the_usual_one_is_read() {
    let mut options = FolderOptions::default();
    options
        .names
        .insert(Role::Scores, "my scores.onnx".to_owned());
    let folder = model_folder(&options);
    let model = read_model(folder.path()).expect("reads");
    assert!(model.file(Role::Scores).ends_with("my scores.onnx"));
}

#[test]
fn thirty_two_structures_are_refused() {
    assert!(why(|o| o.structures.truncate(32)).contains("33"));
}

#[test]
fn a_label_twice_is_refused() {
    why(|o| o.structures[2].label = o.structures[1].label);
}

#[test]
fn a_flip_of_zeros_is_refused() {
    why(|o| o.flip = vec![0; 33]);
}

#[test]
fn a_score_class_beyond_the_groups_is_refused() {
    why(|o| o.score_class[3] = o.score_groups.len() + 1);
}

#[test]
fn a_box_of_one_hundred_is_refused() {
    why(|o| o.box_side = 100);
}

#[test]
fn every_rule_of_the_lists_gives_its_own_reason() {
    let changes: [fn(&mut FolderOptions); 12] = [
        |o| o.structures.truncate(32),
        |o| o.structures[0].label = 1,
        |o| o.structures[2].label = o.structures[1].label,
        |o| o.flip.truncate(32),
        |o| o.flip = (0..33).map(|place| (place + 1) % 33).collect(), // not its own undoing
        |o| o.stage_classes = 1,
        |o| o.areas.truncate(68),
        |o| o.areas[0].label = 5,
        |o| o.cortex = [3, 999],
        |o| o.score_class.truncate(32),
        |o| o.score_class[3] = o.score_groups.len() + 1,
        |o| o.box_side = 100,
    ];
    let reasons: HashSet<_> = changes.into_iter().map(why).collect();
    assert_eq!(reasons.len(), 12, "{reasons:?}");
}

#[test]
fn the_limits_of_the_lists_are_where_the_brief_puts_them() {
    for stage_classes in [2, 16] {
        assert!(
            reads(|o| o.stage_classes = stage_classes),
            "{stage_classes}"
        );
    }
    for box_side in [32, 512] {
        assert!(reads(|o| o.box_side = box_side), "box {box_side}");
    }
    assert!(
        reads(|o| o.score_class = vec![8; 33]),
        "a class may equal the groups"
    );
    for stage_classes in [0, 1, 17] {
        why(|o| o.stage_classes = stage_classes);
    }
    for box_side in [0, 31, 33, 544] {
        why(|o| o.box_side = box_side);
    }
}

#[test]
fn what_is_for_people_in_the_list_is_not_needed() {
    let folder = model_folder(&FolderOptions::default());
    let path = folder.path().join("model.json");
    let mut list: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("read")).expect("json");
    let object = list.as_object_mut().expect("an object");
    object.remove("made");
    object.remove("checked");
    object.insert("added later".to_owned(), serde_json::json!([1, 2, 3]));
    fs::write(&path, serde_json::to_vec(&list).expect("json")).expect("written");
    assert!(read_model(folder.path()).is_ok());
}

#[test]
fn a_sum_matches_the_published_test_vectors() {
    let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let nothing = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    assert_eq!(sum_of(&b"abc"[..]).expect("sum"), abc);
    assert_eq!(sum_of(io::empty()).expect("sum"), nothing);
}

#[test]
fn a_sum_of_more_than_one_block_matches_the_hashing_crate() {
    let bytes: Vec<u8> = (0..2_500_000_u32).map(|n| (n % 251) as u8).collect();
    assert_eq!(sum_of(&bytes[..]).expect("sum"), expected_sum(&bytes));
}

/// Fails once with this kind of error, then gives its bytes.
struct FailsOnce(Option<io::ErrorKind>, &'static [u8]);

impl Read for FailsOnce {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self.0.take() {
            Some(kind) => Err(kind.into()),
            None => self.1.read(buffer),
        }
    }
}

#[test]
fn a_sum_goes_on_after_an_interruption_and_stops_at_a_failure() {
    let interrupted = FailsOnce(Some(io::ErrorKind::Interrupted), b"abc");
    assert_eq!(sum_of(interrupted).expect("sum"), expected_sum(b"abc"));
    let denied = FailsOnce(Some(io::ErrorKind::PermissionDenied), b"abc");
    assert!(sum_of(denied).is_err());
}
