//! A plugin folder is offered only when every file is the one its description lists.

use std::fs;

use serde_json::{Value, json};
use voxa_models::SUPPORTED;

use super::{Limits, check_folder, check_with};
use crate::describe::tests::example;
use crate::testkit::{plugin_folder, sum};
use crate::{DESCRIPTION, PluginError};

/// The bytes of the network file the example names.
const NETWORK: &[u8] = b"made-up network";

/// The example's description without its list of files, for the folder to list its own.
fn unlisted() -> Value {
    let mut description = example();
    description
        .as_object_mut()
        .expect("an object")
        .remove("files");
    description
}

/// What checking a folder made with `description` and `files` says.
fn checked(description: &Value, files: &[(&str, &[u8])]) -> Result<crate::Offered, PluginError> {
    let folder = plugin_folder(description, files);
    check_folder(folder.path(), SUPPORTED)
}

#[test]
fn a_good_folder_is_offered_with_the_sum_of_its_description() {
    let folder = plugin_folder(&unlisted(), &[("strip.onnx", NETWORK)]);
    let offered = check_folder(folder.path(), SUPPORTED).expect("offered");
    let text = fs::read(folder.path().join(DESCRIPTION)).expect("plugin.json");
    assert_eq!(offered.sum, sum(&text));
    assert_eq!(offered.folder, folder.path());
    assert_eq!(offered.description.name, "Example skull strip");
}

#[test]
fn a_folder_that_is_not_there_or_has_no_description_is_refused() {
    let folder = tempfile::tempdir().expect("a folder");
    let gone = folder.path().join("not here");
    assert!(matches!(
        check_folder(&gone, SUPPORTED),
        Err(PluginError::NotFound)
    ));
    let refused = check_folder(folder.path(), SUPPORTED);
    assert!(matches!(refused, Err(PluginError::NoDescription)));
}

#[test]
fn a_description_too_long_is_refused_unread() {
    let folder = tempfile::tempdir().expect("a folder");
    let long = vec![b' '; (1 << 20) + 1];
    fs::write(folder.path().join(DESCRIPTION), long).expect("written");
    let refused = check_folder(folder.path(), SUPPORTED);
    assert!(matches!(refused, Err(PluginError::NoDescription)));
}

#[test]
fn a_description_that_breaks_a_rule_says_which() {
    let mut description = unlisted();
    description["licence"] = json!("");
    let refused = checked(&description, &[("strip.onnx", NETWORK)]).expect_err("refused");
    assert_eq!(refused.to_string(), "The description's licence is empty.");
}

#[test]
fn a_name_with_a_separator_or_dots_is_refused() {
    for name in [
        "../strip.onnx",
        "a\\strip.onnx",
        "c:strip.onnx",
        "sub/strip.onnx",
    ] {
        let mut description = example();
        description["files"] = json!([{ "name": name, "bytes": 1, "sha256": sum(b"x") }]);
        description["network"] = json!(name);
        let refused = checked(&description, &[]).expect_err("refused");
        assert!(
            matches!(refused, PluginError::NotPlain(_)),
            "{name}: {refused:?}"
        );
    }
}

#[test]
fn two_names_differing_only_in_case_are_refused() {
    let mut description = example();
    let entry =
        |name: &str| json!({ "name": name, "bytes": NETWORK.len(), "sha256": sum(NETWORK) });
    description["files"] = json!([entry("strip.onnx"), entry("STRIP.onnx")]);
    let refused = checked(&description, &[("strip.onnx", NETWORK)]).expect_err("refused");
    assert!(matches!(refused, PluginError::Twice(_)), "{refused:?}");
}

#[test]
fn a_network_not_listed_or_not_an_onnx_file_is_refused() {
    let words = "The description's network must be one of its files, a .onnx file.";
    let mut description = unlisted();
    description["network"] = json!("other.onnx");
    let refused = checked(&description, &[("strip.onnx", NETWORK)]).expect_err("refused");
    assert_eq!(refused.to_string(), words);
    description["network"] = json!("strip.bin");
    let refused = checked(&description, &[("strip.bin", NETWORK)]).expect_err("refused");
    assert_eq!(refused.to_string(), words);
}

#[test]
fn a_missing_file_is_missing() {
    let mut description = example();
    description["files"] = json!([{ "name": "strip.onnx", "bytes": 3, "sha256": sum(b"abc") }]);
    let refused = checked(&description, &[]).expect_err("refused");
    assert!(matches!(refused, PluginError::Missing(_)), "{refused:?}");
}

#[test]
fn a_file_one_byte_short_is_the_wrong_size() {
    let mut description = example();
    let entry = json!({ "name": "strip.onnx", "bytes": NETWORK.len() + 1, "sha256": sum(NETWORK) });
    description["files"] = json!([entry]);
    let refused = checked(&description, &[("strip.onnx", NETWORK)]).expect_err("refused");
    assert!(matches!(refused, PluginError::WrongSize(_)), "{refused:?}");
}

#[test]
fn a_file_with_one_byte_changed_has_the_wrong_sum() {
    let mut description = example();
    let entry = json!({ "name": "strip.onnx", "bytes": NETWORK.len(), "sha256": sum(NETWORK) });
    description["files"] = json!([entry]);
    let mut changed = NETWORK.to_vec();
    changed[0] ^= 1;
    let refused = checked(&description, &[("strip.onnx", &changed)]).expect_err("refused");
    assert!(matches!(refused, PluginError::WrongSum(_)), "{refused:?}");
}

#[test]
fn a_folder_where_a_file_should_be_is_not_a_file() {
    let folder = plugin_folder(&unlisted(), &[("strip.onnx", NETWORK)]);
    fs::remove_file(folder.path().join("strip.onnx")).expect("removed");
    fs::create_dir(folder.path().join("strip.onnx")).expect("a folder in its place");
    let refused = check_folder(folder.path(), SUPPORTED).expect_err("refused");
    assert!(matches!(refused, PluginError::NotAFile(_)), "{refused:?}");
}

#[cfg(windows)]
#[test]
fn a_link_is_refused() {
    let folder = plugin_folder(&unlisted(), &[("strip.onnx", NETWORK)]);
    let real = folder.path().join("real.onnx");
    fs::rename(folder.path().join("strip.onnx"), &real).expect("moved");
    let link = folder.path().join("strip.onnx");
    if std::os::windows::fs::symlink_file(&real, &link).is_err() {
        println!("this computer will not make a link without leave; the check is not run");
        return;
    }
    let refused = check_folder(folder.path(), SUPPORTED).expect_err("refused");
    assert!(matches!(refused, PluginError::NotAFile(_)), "{refused:?}");
}

#[test]
fn files_over_the_limits_are_refused() {
    let folder = plugin_folder(&unlisted(), &[("strip.onnx", NETWORK)]);
    let length = NETWORK.len() as u64;
    let one_file = Limits {
        file: length - 1,
        total: length * 4,
    };
    let refused = check_with(folder.path(), SUPPORTED, one_file).expect_err("refused");
    assert!(matches!(refused, PluginError::TooLarge), "{refused:?}");
    let together = Limits {
        file: length,
        total: length - 1,
    };
    let refused = check_with(folder.path(), SUPPORTED, together).expect_err("refused");
    assert!(matches!(refused, PluginError::TooLarge), "{refused:?}");
}

#[test]
fn no_error_sentence_names_a_file() {
    let name = "strip.onnx".to_owned();
    let errors = [
        PluginError::NotPlain(name.clone()),
        PluginError::Twice(name.clone()),
        PluginError::Missing(name.clone()),
        PluginError::NotAFile(name.clone()),
        PluginError::WrongSize(name.clone()),
        PluginError::WrongSum(name.clone()),
    ];
    for error in errors {
        let said = error.to_string();
        assert!(!said.contains(&name), "{said}");
        assert!(said.ends_with('.'), "{said}");
    }
}
