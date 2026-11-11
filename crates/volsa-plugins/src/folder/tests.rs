//! A plugin folder is offered only when every file is the one its description lists.

use std::fs;

use serde_json::{Value, json};
use volsa_models::SUPPORTED;

use super::{Limits, check_folder, check_with};
use crate::describe::tests::example;
use crate::testkit::onnx::{conv_network, image_input, network_bytes};
use crate::testkit::{plugin_folder, sum};
use crate::{DESCRIPTION, PluginError};

/// The bytes of the network file the example names: one convolution, two channels.
fn network() -> Vec<u8> {
    conv_network(&image_input(), &[0.0, 1.0], &[0.0, -0.5])
}

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
    let folder = plugin_folder(&unlisted(), &[("strip.onnx", &network())]);
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
    let refused = checked(&description, &[("strip.onnx", &network())]).expect_err("refused");
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
        |name: &str| json!({ "name": name, "bytes": network().len(), "sha256": sum(&network()) });
    description["files"] = json!([entry("strip.onnx"), entry("STRIP.onnx")]);
    let refused = checked(&description, &[("strip.onnx", &network())]).expect_err("refused");
    assert!(matches!(refused, PluginError::Twice(_)), "{refused:?}");
}

#[test]
fn a_network_not_listed_or_not_an_onnx_file_is_refused() {
    let words = "The description's network must be one of its files, a .onnx file.";
    let mut description = unlisted();
    description["network"] = json!("other.onnx");
    let refused = checked(&description, &[("strip.onnx", &network())]).expect_err("refused");
    assert_eq!(refused.to_string(), words);
    description["network"] = json!("strip.bin");
    let refused = checked(&description, &[("strip.bin", &network())]).expect_err("refused");
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
    let entry =
        json!({ "name": "strip.onnx", "bytes": network().len() + 1, "sha256": sum(&network()) });
    description["files"] = json!([entry]);
    let refused = checked(&description, &[("strip.onnx", &network())]).expect_err("refused");
    assert!(matches!(refused, PluginError::WrongSize(_)), "{refused:?}");
}

#[test]
fn a_file_with_one_byte_changed_has_the_wrong_sum() {
    let mut description = example();
    let entry =
        json!({ "name": "strip.onnx", "bytes": network().len(), "sha256": sum(&network()) });
    description["files"] = json!([entry]);
    let mut changed = network();
    changed[0] ^= 1;
    let refused = checked(&description, &[("strip.onnx", &changed)]).expect_err("refused");
    assert!(matches!(refused, PluginError::WrongSum(_)), "{refused:?}");
}

#[test]
fn a_folder_where_a_file_should_be_is_not_a_file() {
    let folder = plugin_folder(&unlisted(), &[("strip.onnx", &network())]);
    fs::remove_file(folder.path().join("strip.onnx")).expect("removed");
    fs::create_dir(folder.path().join("strip.onnx")).expect("a folder in its place");
    let refused = check_folder(folder.path(), SUPPORTED).expect_err("refused");
    assert!(matches!(refused, PluginError::NotAFile(_)), "{refused:?}");
}

#[cfg(windows)]
#[test]
fn a_link_is_refused() {
    let folder = plugin_folder(&unlisted(), &[("strip.onnx", &network())]);
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
    let folder = plugin_folder(&unlisted(), &[("strip.onnx", &network())]);
    let length = network().len() as u64;
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

#[test]
fn a_network_with_an_operator_outside_standard_onnx_is_refused() {
    let spec = crate::onnx::tests::one_node("Mystery", "com.example", vec![(String::new(), 17)]);
    let bytes = network_bytes(&spec);
    let refused = checked(&unlisted(), &[("strip.onnx", &bytes)]).expect_err("refused");
    let words = "The network uses operators outside standard ONNX, which Volsa does not run.";
    assert_eq!(refused.to_string(), words);
}

#[test]
fn an_offered_folder_says_what_its_network_takes_and_gives() {
    let offered = checked(&unlisted(), &[("strip.onnx", &network())]).expect("offered");
    assert_eq!(offered.network.inputs[0].name, "image");
    assert_eq!(offered.network.outputs[0].name, "scores");
}

#[test]
fn a_later_kind_is_refused_when_offered() {
    let mut description = unlisted();
    description["kind"] = json!("value-map");
    description["output"] = json!({ "unit": "ms", "range": [0, 3000] });
    let refused = checked(&description, &[("strip.onnx", &network())]).expect_err("refused");
    let words = "This version of Volsa cannot run plugins that give a value map yet.";
    assert_eq!(refused.to_string(), words);
}

#[test]
fn an_offered_folder_is_wired_to_its_network() {
    let offered = checked(&unlisted(), &[("strip.onnx", &network())]).expect("offered");
    assert_eq!(offered.wiring.output, "scores");
    assert_eq!(offered.wiring.free.len(), 3);
}

#[test]
fn a_network_that_does_not_fit_its_description_is_refused() {
    let three = conv_network(&image_input(), &[0.0, 1.0, 2.0], &[0.0, 0.0, 0.0]);
    let refused = checked(&unlisted(), &[("strip.onnx", &three)]).expect_err("refused");
    let words = "The network's answer is not the size its description gives.";
    assert_eq!(refused.to_string(), words);
}
