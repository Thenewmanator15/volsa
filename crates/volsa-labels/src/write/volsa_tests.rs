//! Who made a label set, as the `volsa` object of a table keeps it, and the model's check as
//! the `qc` object keeps it: cut when written, forgiving when read, and never able to spoil
//! a table.

use proptest::collection::{hash_map, vec};
use proptest::prelude::*;
use serde_json::{Value, json};
use volsa_core::Affine;

use super::table_tests::{head, made};
use super::write_table;
use crate::made::Made;
use crate::nifti::LabelGrid;
use crate::set::label_set;
use crate::table::{Table, Verdict, read_table};

/// A table of one name with this as its `volsa` value.
fn with_volsa(volsa: &Value) -> Table {
    let table = json!({"regions": {"thalamus": {"ids": {"left": 10}}}, "volsa": volsa});
    read_table("regions.json", &table.to_string()).expect("a table whatever the volsa value is")
}

fn written_volsa(made: Made) -> Value {
    let table = Table {
        made: Some(made),
        ..head()
    };
    let text = write_table(&table).expect("writes");
    serde_json::from_str::<Value>(&text).expect("json")["volsa"].clone()
}

#[test]
fn a_table_with_no_volsa_object_or_one_that_is_not_an_object_has_no_maker() {
    let bare = r#"{"regions": {"thalamus": {"ids": {"left": 10}}}}"#;
    assert_eq!(read_table("a.json", bare).expect("reads").made, None);
    for odd in [
        json!(null),
        json!(5),
        json!("Volsa"),
        json!([1, 2]),
        json!(true),
    ] {
        assert_eq!(with_volsa(&odd).made, None, "{odd}");
    }
    let plain = r#"{"parts": {"1": "cochlea"}}"#;
    assert_eq!(read_table("a.json", plain).expect("reads").made, None);
    let slicer = read_table("a.ctbl", "17 hippocampus_(left) 220 216 20 255\n");
    assert_eq!(slicer.expect("reads").made, None);
}

#[test]
fn a_volsa_object_is_read_part_by_part() {
    let volsa = json!({
        "by": "Volsa 9.8.7", "model": "SynthSeg 2.0", "model_sum": "ab".repeat(32),
        "device": "the processor", "when": "2099-01-02 03:04", "notes": ["One.", "Two."]
    });
    let read = with_volsa(&volsa).made.expect("there is one");
    let expected = Made {
        by: "Volsa 9.8.7".to_owned(),
        model: "SynthSeg 2.0".to_owned(),
        model_sum: "ab".repeat(32),
        device: "the processor".to_owned(),
        when: "2099-01-02 03:04".to_owned(),
        notes: vec!["One.".to_owned(), "Two.".to_owned()],
    };
    assert_eq!(read, expected);
    assert_eq!(
        with_volsa(&json!({})).made,
        Some(Made::default()),
        "an empty object is a maker"
    );
}

#[test]
fn a_long_volsa_object_is_cut_to_what_is_kept() {
    let notes: Vec<String> = (1..=20).map(|at| format!("Note {at}")).collect();
    let volsa = json!({"by": "b".repeat(500), "model_sum": "x", "notes": notes});
    let read = with_volsa(&volsa).made.expect("there is one");
    assert_eq!(read.by, "b".repeat(80));
    assert_eq!(read.model_sum, "", "x is no sum");
    assert_eq!(read.notes.len(), 8);
    assert_eq!(read.notes.last().map(String::as_str), Some("Note 8"));
    let long = json!({
        "model": "m".repeat(81), "device": "d".repeat(81), "when": "w".repeat(81),
        "notes": ["n".repeat(301)]
    });
    let read = with_volsa(&long).made.expect("there is one");
    let lengths = [
        read.model.len(),
        read.device.len(),
        read.when.len(),
        read.notes[0].len(),
    ];
    assert_eq!(lengths, [80, 80, 80, 300]);
}

#[test]
fn text_is_cut_between_characters_never_inside_one() {
    let two_bytes = "é".repeat(100);
    let three_bytes = "日".repeat(400);
    let read = with_volsa(&json!({"by": two_bytes, "notes": [three_bytes]}))
        .made
        .expect("there");
    assert_eq!(read.by, "é".repeat(80));
    assert_eq!(read.notes, ["日".repeat(300)]);
    let written = written_volsa(Made {
        by: "é".repeat(100),
        ..Made::default()
    });
    assert_eq!(written["by"], "é".repeat(80).as_str());
}

#[test]
fn a_part_that_is_missing_or_of_the_wrong_kind_is_taken_as_absent() {
    let volsa = json!({
        "by": 5, "model": ["a"], "model_sum": 7, "device": null, "when": {"at": 1},
        "notes": "not a list"
    });
    assert_eq!(with_volsa(&volsa).made, Some(Made::default()));
    let mixed = json!({"notes": [1, "kept", null, ["no"], "also kept"]});
    let notes = with_volsa(&mixed).made.expect("there is one").notes;
    assert_eq!(notes, ["kept", "also kept"]);
}

#[test]
fn a_sum_is_sixty_four_hexadecimal_digits_or_nothing() {
    let sum = |text: &str| {
        with_volsa(&json!({"model_sum": text}))
            .made
            .expect("there")
            .model_sum
    };
    let good = "0123456789abcdef".repeat(4);
    assert_eq!(sum(&good), good);
    assert_eq!(sum(&good.to_uppercase()), good, "kept in lower case");
    assert_eq!(sum(&good[1..]), "", "sixty-three digits");
    assert_eq!(sum(&format!("{good}0")), "", "sixty-five digits");
    assert_eq!(
        sum(&format!("g{}", &good[1..])),
        "",
        "a digit that is not hexadecimal"
    );
    assert_eq!(
        sum(&"é".repeat(32)),
        "",
        "sixty-four bytes of letters that are not digits"
    );
    assert_eq!(sum(""), "");
}

#[test]
fn what_is_written_of_who_made_it_is_cut_and_leaves_out_what_is_not_there() {
    let mut long = made();
    long.by = "b".repeat(500);
    long.model_sum = "x".to_owned();
    long.notes = (1..=20).map(|at| "n".repeat(300 + at)).collect();
    let written = written_volsa(long);
    assert_eq!(written["by"], "b".repeat(80).as_str());
    assert_eq!(written.get("model_sum"), None, "left out");
    let notes = written["notes"].as_array().expect("a list");
    assert_eq!(notes.len(), 8);
    assert!(
        notes
            .iter()
            .all(|note| note.as_str().map(|n| n.chars().count()) == Some(300))
    );
    let empty = written_volsa(Made::default());
    assert_eq!(
        empty,
        json!({}),
        "a maker with nothing to say is an empty object"
    );
    let upper = Made {
        model_sum: "AB".repeat(32),
        ..Made::default()
    };
    assert_eq!(written_volsa(upper)["model_sum"], "ab".repeat(32).as_str());
}

#[test]
fn who_made_a_table_is_read_back_when_it_is_within_what_is_kept() {
    for who in [
        made(),
        Made::default(),
        Made {
            notes: vec!["x".repeat(300); 8],
            ..made()
        },
    ] {
        let table = Table {
            made: Some(who),
            ..head()
        };
        let text = write_table(&table).expect("writes");
        assert_eq!(read_table("a.json", &text).expect("reads"), table);
    }
}

#[test]
fn the_models_check_is_read_back_whatever_it_says() {
    let check = |verdict: Verdict| {
        let table = Table {
            verdict: Some(verdict),
            made: None,
            ..head()
        };
        let text = write_table(&table).expect("writes");
        read_table("a.json", &text).expect("reads").verdict
    };
    let passed = Verdict {
        passed: true,
        fail_below: 0.65,
        low: Vec::new(),
    };
    assert_eq!(check(passed.clone()), Some(passed));
    let failed = Verdict {
        passed: false,
        fail_below: 0.65,
        low: vec![("a".to_owned(), 0.5), ("b".to_owned(), 0.62)],
    };
    assert_eq!(check(failed.clone()), Some(failed));
    // A check that failed with no part to blame, and one that passed with one named.
    let unplaced = Verdict {
        passed: false,
        fail_below: 0.7,
        low: Vec::new(),
    };
    assert_eq!(check(unplaced.clone()), Some(unplaced));
    // A group with no score is written with none, and comes back as no number.
    let unscored = Verdict {
        passed: false,
        fail_below: 0.65,
        low: vec![("a".to_owned(), 0.5), ("b".to_owned(), f64::NAN)],
    };
    let read = check(unscored).expect("there is one");
    assert!(!read.passed);
    assert_eq!(read.low.len(), 2);
    assert_eq!(read.low[0], ("a".to_owned(), 0.5));
    assert!(read.low[1].0 == "b" && read.low[1].1.is_nan());
    // A mark that is no number is no check at all, and nothing of one is written.
    let marked = Verdict {
        passed: true,
        fail_below: f64::NAN,
        low: Vec::new(),
    };
    assert_eq!(check(marked.clone()), None);
    let table = Table {
        verdict: Some(marked),
        ..head()
    };
    assert!(!write_table(&table).expect("writes").contains("\"qc\""));
}

#[test]
fn a_label_set_carries_who_made_it_from_its_table() {
    let grid = LabelGrid {
        dims: [2, 1, 1],
        ids: vec![17, 53],
        placement: Affine::identity(),
    };
    let table = head();
    let set = label_set(&grid, Some(&table)).expect("fits");
    assert_eq!(set.made, table.made);
    assert!(set.made.is_some());
    assert_eq!(label_set(&grid, None).expect("fits").made, None);
}

#[test]
fn a_score_a_file_leaves_empty_is_a_group_that_was_not_scored() {
    let text = r#"{"regions": {"a": {"ids": {"left": 1}}},
        "qc": {"scores": {"a": null, "b": 0.9}, "fail_below": 0.65}}"#;
    let verdict = read_table("a.json", text)
        .expect("reads")
        .verdict
        .expect("there is one");
    assert!(!verdict.passed);
    assert_eq!(verdict.low.len(), 1);
    assert!(verdict.low[0].0 == "a" && verdict.low[0].1.is_nan());
}

#[test]
fn an_infinite_score_is_written_empty_and_read_as_a_group_that_failed() {
    let verdict = Verdict {
        passed: false,
        fail_below: 0.65,
        low: vec![
            ("a".to_owned(), f64::NEG_INFINITY),
            ("b".to_owned(), 0.5),
            ("c".to_owned(), f64::INFINITY),
        ],
    };
    let table = Table {
        verdict: Some(verdict),
        made: None,
        ..head()
    };
    let text = write_table(&table).expect("writes");
    let written: Value = serde_json::from_str(&text).expect("json");
    // Neither infinity is a number a file can hold: each is written empty.
    let scores = json!({"a": null, "b": 0.5, "c": null});
    assert_eq!(written["qc"]["scores"], scores);
    assert_eq!(written["qc"]["low"], json!(["a", "b", "c"]));
    // Read back, each is a group that was not scored, which is a group that failed.
    let read = read_table("a.json", &text)
        .expect("reads")
        .verdict
        .expect("there is one");
    assert!(!read.passed);
    let names: Vec<&str> = read.low.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["a", "b", "c"]);
    assert!(read.low[0].1.is_nan() && read.low[2].1.is_nan());
    assert_eq!(read.low[1].1, 0.5);
    // It is the file's empty score that fails it, not only the word `passed`.
    let unsaid = text.replace("\"passed\": false,", "");
    assert!(!unsaid.contains("passed"), "the word is gone");
    let read = read_table("a.json", &unsaid)
        .expect("reads")
        .verdict
        .expect("there is one");
    assert!(!read.passed);
    assert_eq!(read.low.len(), 3);
}

#[test]
fn a_quote_a_backslash_a_new_line_and_another_script_in_who_made_it_come_back() {
    let who = Made {
        by: "Volsa \"9.8.7\", a back\\slash,\na new line, é and 日本語".to_owned(),
        notes: vec![
            "One \"quoted\" word, a back\\slash,\nand a new line.".to_owned(),
            "été 日本語 \u{1F9E0}".to_owned(),
        ],
        ..made()
    };
    let table = Table {
        made: Some(who),
        ..head()
    };
    let text = write_table(&table).expect("writes");
    assert_eq!(read_table("a.json", &text).expect("reads"), table);
}

/// Any JSON value, with the keys of a `volsa` object among the keys of its objects.
fn any_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        ".{0,400}".prop_map(Value::from),
    ];
    leaf.prop_recursive(3, 40, 6, |inner| {
        let keys = "by|model|model_sum|device|when|notes|[a-z]{0,3}";
        prop_oneof![
            vec(inner.clone(), 0..12).prop_map(Value::Array),
            hash_map(keys, inner, 0..8).prop_map(|map| Value::Object(map.into_iter().collect())),
        ]
    })
}

proptest! {
    #[test]
    fn whatever_the_volsa_value_is_the_table_reads_and_what_is_kept_is_within_its_limits(
        volsa in any_json()
    ) {
        if let Some(made) = with_volsa(&volsa).made {
            for line in [&made.by, &made.model, &made.device, &made.when] {
                prop_assert!(line.chars().count() <= 80);
            }
            prop_assert!(made.notes.len() <= 8);
            prop_assert!(made.notes.iter().all(|note| note.chars().count() <= 300));
            let is_sum = made.model_sum.len() == 64
                && made.model_sum.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
            prop_assert!(made.model_sum.is_empty() || is_sum);
        } else {
            prop_assert!(!volsa.is_object());
        }
    }
}
