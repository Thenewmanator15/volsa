//! A description that breaks a rule is refused, and the sentence names the field.

use serde_json::{Value, json};

use super::tests::{example, read};
use crate::read_description;

/// The sentence a description refused after `change` is said with.
fn refused(change: impl FnOnce(&mut Value)) -> String {
    let mut value = example();
    change(&mut value);
    read(&value).expect_err("refused").to_string()
}

/// Adds a second name to the output's names.
fn push_name(value: &mut Value, name: Value) {
    let names = value["output"]["names"].as_array_mut().expect("a list");
    names.push(name);
}

/// Takes a key out of an object of the JSON.
fn without(object: &mut Value, key: &str) {
    object.as_object_mut().expect("an object").remove(key);
}

#[test]
fn text_that_is_not_json_is_not_a_description() {
    let said = read_description(b"not json")
        .expect_err("refused")
        .to_string();
    assert_eq!(
        said,
        "This folder's plugin.json is not a description Voxa can read."
    );
    let said = read_description(b"[1, 2]")
        .expect_err("refused")
        .to_string();
    assert_eq!(
        said,
        "This folder's plugin.json is not a description Voxa can read."
    );
}

#[test]
fn format_two_is_refused_whatever_else_it_holds() {
    let said = read_description(br#"{"format": 2, "anything": [1]}"#).expect_err("refused");
    let words = "This plugin is in format 2, which this version of Voxa cannot read.";
    assert_eq!(said.to_string(), words);
}

#[test]
fn a_description_without_a_scaling_names_input_scaling() {
    let said = refused(|value| without(&mut value["input"], "scaling"));
    assert_eq!(said, "The description's input.scaling is missing.");
}

#[test]
fn an_unknown_field_is_refused() {
    let said = refused(|value| value["colour_scheme"] = json!("dark"));
    assert_eq!(
        said,
        "The description's colour_scheme is not a field Voxa knows."
    );
    let said = refused(|value| value["input"]["fit"]["padding"] = json!(3));
    assert_eq!(
        said,
        "The description's input.fit.padding is not a field Voxa knows."
    );
}

#[test]
fn an_unknown_kind_is_refused() {
    let said = refused(|value| value["kind"] = json!("segmentation"));
    assert_eq!(said, "The description's kind is not one Voxa knows.");
}

#[test]
fn a_name_with_a_right_to_left_mark_or_a_line_break_is_refused() {
    for name in ["Brain\u{202E}niarB", "Brain\nstrip", "Brain\u{200B}"] {
        let said = refused(|value| value["name"] = json!(name));
        assert_eq!(
            said,
            "The description's name holds a character Voxa does not show."
        );
    }
}

#[test]
fn text_longer_than_its_limit_or_empty_is_refused() {
    let said = refused(|value| value["name"] = json!("n".repeat(61)));
    assert_eq!(said, "The description's name is too long.");
    let said = refused(|value| value["licence"] = json!(""));
    assert_eq!(said, "The description's licence is empty.");
    let said = refused(|value| value["limits"][0] = json!("l".repeat(301)));
    assert_eq!(said, "The description's limits is too long.");
    let said = refused(|value| value["limits"] = json!(vec!["A limit."; 9]));
    assert_eq!(
        said,
        "The description's limits holds more than 8 sentences."
    );
}

#[test]
fn an_id_must_be_lower_case_letters_digits_dots_and_dashes() {
    for id in ["Example.Strip", "ab", "has space", "a/b"] {
        let said = refused(|value| value["id"] = json!(id));
        let words = "The description's id must be 3 to 64 of a-z, 0-9, dots and dashes.";
        assert_eq!(said, words);
    }
}

#[test]
fn a_sum_that_is_not_sixty_four_hex_digits_is_refused() {
    let said = refused(|value| value["files"][0]["sha256"] = json!("abc"));
    assert_eq!(said, "The description's files.sha256 is not a SHA-256 sum.");
}

#[test]
fn axes_must_name_each_pair_once() {
    for axes in ["LRA", "LIAS", "lia", "LIX"] {
        let said = refused(|value| value["input"]["axes"] = json!(axes));
        let words = "The description's input.axes must be three letters, one each of L or R, \
                     A or P, and S or I.";
        assert_eq!(said, words);
    }
}

#[test]
fn a_spacing_or_a_side_that_is_not_above_nought_is_refused() {
    let said = refused(|value| value["input"]["spacing_mm"] = json!([1, 0, 1]));
    assert_eq!(
        said,
        "The description's input.spacing_mm must be three numbers above 0."
    );
    let said = refused(|value| value["input"]["fit"]["whole"] = json!([256, 0, 256]));
    let words = "The description's input.fit.whole must be three whole numbers from 1 to 1024.";
    assert_eq!(said, words);
}

#[test]
fn a_step_longer_than_its_block_is_refused() {
    let said = refused(|value| {
        value["input"]["fit"] = json!({
            "blocks": { "size": [64, 64, 64], "step": [32, 65, 32] },
            "outside": "zero"
        });
        value["input"]["scaling"] = json!({ "kind": "none" });
    });
    let words = "The description's input.fit.blocks.step must be from 1 to the block's size.";
    assert_eq!(said, words);
}

#[test]
fn a_fit_needs_exactly_one_of_whole_and_blocks() {
    let said = refused(|value| value["input"]["fit"] = json!({ "outside": "lowest" }));
    assert_eq!(
        said,
        "The description's input.fit must hold either whole or blocks."
    );
}

#[test]
fn grid_statistics_need_a_whole_box_and_the_lowest_outside() {
    let words = "The description's input.scaling.over can be grid only with a whole box and \
                 the lowest value outside.";
    let said = refused(|value| value["input"]["fit"]["outside"] = json!("zero"));
    assert_eq!(said, words);
    let said = refused(|value| {
        value["input"]["fit"] = json!({
            "blocks": { "size": [64, 64, 64], "step": [32, 32, 32] },
            "outside": "lowest"
        });
    });
    assert_eq!(said, words);
}

#[test]
fn percentiles_out_of_order_are_refused() {
    let said = refused(|value| value["input"]["scaling"]["low"] = json!(99));
    let words = "The description's input.scaling must have 0 <= low < high <= 100.";
    assert_eq!(said, words);
}

#[test]
fn dividing_by_nought_is_refused() {
    let said = refused(|value| {
        value["input"]["scaling"] =
            json!({ "kind": "fixed", "clip": [0, 1], "subtract": 0, "divide": 0 });
    });
    assert_eq!(
        said,
        "The description's input.scaling.divide must not be 0."
    );
}

#[test]
fn a_channel_label_without_a_name_is_refused() {
    let said = refused(|value| value["output"]["channels"] = json!([0, 2]));
    assert_eq!(
        said,
        "The description's output.channels names a label with no name."
    );
}

#[test]
fn names_sharing_an_id_or_a_name_and_side_are_refused() {
    let said = refused(|value| {
        push_name(
            value,
            json!({ "id": 1, "name": "Skull", "side": "midline", "group": "Head" }),
        );
    });
    assert_eq!(said, "The description's output.names gives one id twice.");
    let said = refused(|value| {
        push_name(
            value,
            json!({ "id": 2, "name": "Brain", "side": "midline", "group": "Head" }),
        );
    });
    assert_eq!(
        said,
        "The description's output.names gives one name and side twice."
    );
}

#[test]
fn a_contrast_voxa_does_not_know_is_refused() {
    let said = refused(|value| value["accepts"]["contrast"] = json!(["T7"]));
    assert_eq!(
        said,
        "The description's accepts.contrast names a contrast Voxa does not know."
    );
}

#[test]
fn a_modality_list_must_not_be_empty() {
    let said = refused(|value| value["accepts"]["modality"] = json!([]));
    assert_eq!(said, "The description's accepts.modality is empty.");
}

#[test]
fn a_kind_needs_its_own_output() {
    let said = refused(|value| value["kind"] = json!("number"));
    assert_eq!(said, "The description's output.name is missing.");
}
