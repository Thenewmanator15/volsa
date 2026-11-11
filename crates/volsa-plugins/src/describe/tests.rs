//! A description that is right reads as it says.

use serde_json::{Value, json};
use volsa_segment::Toward::{Anterior, Inferior, Left, Right, Superior};

use crate::{
    Contrast, Contrasts, DescribeError, Description, Fit, Gives, Kind, Layout, Moves, Output,
    Outside, Over, Scaling, Side, Voxels, read_description,
};

/// A made-up sum: no real file's.
pub(crate) const SUM: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// The example of the design, section 6.2, with a made-up sum.
pub(crate) fn example() -> Value {
    json!({
        "format": 1,
        "kind": "label-map",
        "id": "example.skull-strip",
        "name": "Example skull strip",
        "version": "1.0",
        "made_by": "An example lab",
        "source": "https://example.org/skull-strip",
        "licence": "MIT",
        "about": "Marks the brain inside the skull on an MRI scan of the head.",
        "limits": ["Trained on adult heads only."],
        "files": [{ "name": "strip.onnx", "bytes": 612_345, "sha256": SUM }],
        "network": "strip.onnx",
        "accepts": { "modality": ["MR"], "contrast": "any", "body_part": "head" },
        "input": {
            "spacing_mm": [1, 1, 1],
            "axes": "LIA",
            "layout": "channels-first",
            "fit": { "whole": [256, 256, 256], "outside": "lowest" },
            "scaling": { "kind": "percentiles", "low": 2, "high": 98, "over": "grid" }
        },
        "output": {
            "gives": "scores",
            "channels": [0, 1],
            "keep_largest_piece": true,
            "names": [{ "id": 1, "name": "Brain", "side": "midline", "group": "Brain" }]
        }
    })
}

/// What a description in this JSON reads as.
pub(crate) fn read(value: &Value) -> Result<Description, DescribeError> {
    read_description(value.to_string().as_bytes())
}

#[test]
fn the_example_of_the_spec_reads() {
    let read = read(&example()).expect("the example reads");
    assert_eq!(read.kind, Kind::LabelMap);
    assert_eq!(read.id, "example.skull-strip");
    assert_eq!(read.limits, ["Trained on adult heads only."]);
    assert_eq!(read.files[0].sha256, SUM);
    assert_eq!(read.files[0].bytes, 612_345);
    assert_eq!(read.network, "strip.onnx");
    assert_eq!(read.accepts.modality, ["MR"]);
    assert_eq!(read.accepts.contrast, Contrasts::Any);
    assert_eq!(read.accepts.body_part, "head");
    assert_eq!(read.input.spacing_mm, [1.0, 1.0, 1.0]);
    assert_eq!(read.input.axes, [Left, Inferior, Anterior]);
    assert_eq!(read.input.layout, Layout::ChannelsFirst);
    assert_eq!(read.input.fit, Fit::Whole([256, 256, 256]));
    assert_eq!(read.input.outside, Outside::Lowest);
    let scaling = Scaling::Percentiles {
        low: 2.0,
        high: 98.0,
        over: Over::Grid,
    };
    assert_eq!(read.input.scaling, scaling);
    let Output::Labels(output) = &read.output else {
        panic!("a label map gives labels");
    };
    assert_eq!(
        output.gives,
        Gives::Scores {
            channels: vec![0, 1]
        }
    );
    assert!(output.keep_largest_piece);
    assert_eq!(output.from, None);
    assert_eq!(output.names[0].id, 1);
    assert_eq!(output.names[0].name, "Brain");
    assert_eq!(output.names[0].side, Side::Midline);
    assert_eq!(output.names[0].colour, None);
    assert_eq!(read.runnable(), Ok(()));
}

#[test]
fn each_later_kind_reads_and_is_not_yet_runnable() {
    let later = [
        (
            "number",
            json!({ "name": "Volume", "unit": "ml", "decimals": 1, "range": [0, 2000] }),
            "one number",
        ),
        (
            "value-map",
            json!({ "unit": "ms", "range": [0, 3000], "from": "map" }),
            "a value map",
        ),
        (
            "alignment",
            json!({ "gives": "field", "unit": "mm" }),
            "an alignment",
        ),
    ];
    for (kind, output, words) in later {
        let mut value = example();
        value["kind"] = json!(kind);
        value["output"] = output;
        let read = read(&value).expect("a later kind reads");
        let refused = read.runnable().expect_err("a later kind does not run yet");
        let said = format!("This version of Volsa cannot run plugins that give {words} yet.");
        assert_eq!(refused.to_string(), said);
    }
}

#[test]
fn an_alignment_may_give_a_matrix() {
    let mut value = example();
    value["kind"] = json!("alignment");
    value["output"] = json!({ "gives": "matrix" });
    let read = read(&value).expect("reads");
    let Output::Alignment(output) = read.output else {
        panic!("an alignment");
    };
    assert_eq!(output.gives, Moves::Matrix);
}

#[test]
fn a_sum_in_capitals_is_kept_in_lower_case() {
    let mut value = example();
    value["files"][0]["sha256"] = json!(SUM.to_ascii_uppercase());
    assert_eq!(read(&value).expect("reads").files[0].sha256, SUM);
}

#[test]
fn blocks_read_with_their_step_and_a_scan_wide_z_score() {
    let mut value = example();
    value["input"]["axes"] = json!("RAS");
    value["input"]["layout"] = json!("channels-last");
    value["input"]["fit"] = json!({
        "blocks": { "size": [128, 112, 112], "step": [64, 56, 56] },
        "outside": "zero"
    });
    value["input"]["scaling"] =
        json!({ "kind": "z-score", "voxels": "above-zero", "over": "scan" });
    let read = read(&value).expect("reads");
    assert_eq!(read.input.axes, [Right, Anterior, Superior]);
    assert_eq!(read.input.layout, Layout::ChannelsLast);
    let blocks = Fit::Blocks {
        size: [128, 112, 112],
        step: [64, 56, 56],
    };
    assert_eq!(read.input.fit, blocks);
    assert_eq!(read.input.outside, Outside::Zero);
    let z = Scaling::ZScore {
        voxels: Voxels::AboveZero,
        over: Over::Scan,
    };
    assert_eq!(read.input.scaling, z);
}

#[test]
fn fixed_and_no_scaling_read() {
    let mut value = example();
    value["input"]["scaling"] =
        json!({ "kind": "fixed", "clip": [-1024, 276], "subtract": -370, "divide": 436.6 });
    let fixed = Scaling::Fixed {
        clip: [-1024.0, 276.0],
        subtract: -370.0,
        divide: 436.6,
    };
    assert_eq!(read(&value).expect("reads").input.scaling, fixed);
    value["input"]["scaling"] = json!({ "kind": "none" });
    assert_eq!(read(&value).expect("reads").input.scaling, Scaling::None);
}

#[test]
fn contrasts_chances_and_labels_read() {
    let mut value = example();
    value["accepts"]["contrast"] = json!(["T1", "3D T1", "FLAIR", "TOF"]);
    value["accepts"]["thickest_mm"] = json!(3);
    let contrasts = Contrasts::Only(vec![
        Contrast::T1,
        Contrast::T1In3d,
        Contrast::Flair,
        Contrast::Tof,
    ]);
    let read_once = read(&value).expect("reads");
    assert_eq!(read_once.accepts.contrast, contrasts);
    assert_eq!(read_once.accepts.thickest_mm, Some(3.0));
    value["output"]["gives"] = json!("chance");
    value["output"]["label"] = json!(1);
    value["output"]["threshold"] = json!(0.5);
    value["output"]
        .as_object_mut()
        .expect("object")
        .remove("channels");
    let Output::Labels(output) = read(&value).expect("reads").output else {
        panic!("labels");
    };
    let chance = Gives::Chance {
        label: 1,
        threshold: 0.5,
    };
    assert_eq!(output.gives, chance);
    value["output"]["gives"] = json!("labels");
    value["output"]["from"] = json!("labels");
    for key in ["label", "threshold"] {
        value["output"].as_object_mut().expect("object").remove(key);
    }
    value["output"]["names"][0]["colour"] = json!([200, 10, 30]);
    let Output::Labels(output) = read(&value).expect("reads").output else {
        panic!("labels");
    };
    assert_eq!(output.gives, Gives::Labels);
    assert_eq!(output.from.as_deref(), Some("labels"));
    assert_eq!(output.names[0].colour, Some([200, 10, 30]));
}

#[test]
fn every_contrast_has_its_word() {
    let words: Vec<&str> = Contrast::ALL
        .iter()
        .map(|contrast| contrast.word())
        .collect();
    let expected = [
        "T1",
        "3D T1",
        "T2",
        "PD",
        "FLAIR",
        "T2*",
        "Diffusion",
        "ADC map",
        "SWI",
        "STIR",
        "Angiography (TOF)",
    ];
    assert_eq!(words, expected);
}
