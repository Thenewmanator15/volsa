//! Writing a table of names: the shape the pipeline writes, that it reads back as it was,
//! and the tables it refuses.

use serde_json::{Value, json};

use super::{WriteError, write_table};
use crate::made::Made;
use crate::table::{Entry, Side, Table, Trust, Verdict, colour_for, read_table};

const HIPPOCAMPUS: [u8; 3] = [0xdc, 0xd8, 0x14];
const STEM: [u8; 3] = [0x77, 0x9f, 0xb0];

pub(super) fn entry(name: &str, side: Side, id: u32, trust: Trust) -> Entry {
    Entry {
        name: name.to_owned(),
        group: "Deep brain".to_owned(),
        side,
        ids: vec![id],
        colour: colour_for(id),
        trust,
    }
}

/// Who made the labels of a made-up head.
pub(super) fn made() -> Made {
    Made {
        by: "Voxa 9.8.7".to_owned(),
        model: "SynthSeg 2.0, standard model".to_owned(),
        model_sum: "0123456789abcdef".repeat(4),
        device: "the processor".to_owned(),
        when: "2099-01-02 03:04".to_owned(),
        notes: vec![
            "The slices of this scan are 6 mm apart.".to_owned(),
            "The scan was cut to the head.".to_owned(),
        ],
    }
}

/// A table as the pipeline would write it: a pair with one colour, a midline name, a pair
/// that is approximate with colours of the numbers' own, a failed check and who made it.
pub(super) fn head() -> Table {
    let model = |name: &str, side, id| entry(name, side, id, Trust::Model);
    let coloured = |entry: Entry, colour| Entry { colour, ..entry };
    let approximate = |name: &str, side, id| Entry {
        group: "Outside the brain".to_owned(),
        ..entry(name, side, id, Trust::Approximate("Rules.".to_owned()))
    };
    Table {
        title: "A made-up head".to_owned(),
        entries: vec![
            coloured(model("hippocampus", Side::Left, 17), HIPPOCAMPUS),
            coloured(model("hippocampus", Side::Right, 53), HIPPOCAMPUS),
            Entry {
                group: "Cerebellum and brainstem".to_owned(),
                ..coloured(model("brain stem", Side::Midline, 16), STEM)
            },
            approximate("eyeball", Side::Left, 101),
            approximate("eyeball", Side::Right, 102),
        ],
        verdict: Some(Verdict {
            passed: false,
            fail_below: 0.65,
            low: vec![("general csf".to_owned(), 0.58)],
        }),
        made: Some(made()),
    }
}

fn round_trip(table: &Table) -> Table {
    let text = write_table(table).expect("writes");
    read_table("regions.json", &text).expect("reads")
}

#[test]
fn a_table_is_read_back_as_it_was_written() {
    let table = head();
    assert_eq!(round_trip(&table), table);
}

/// A table as 0.4.0 reads it from the labelling pipeline: no `voxa` object.
const PIPELINE: &str = r##"{
 "title": "A made-up head",
 "model": "robust",
 "regions": {
  "hippocampus": {"group": "Deep brain", "colour": "#dcd814", "ids": {"left": 17, "right": 53}, "ml": {"left": 1.0, "right": 1.0}},
  "brain stem": {"group": "Cerebellum and brainstem", "colour": "#779fb0", "ids": {"midline": 16}},
  "eyeball": {"group": "Outside the brain", "colour": "#3cb4e6", "ids": {"left": 101, "right": 102}, "approximate": true, "how": "Rules."}
 },
 "qc": {"scores": {"general white matter": 0.8, "general csf": 0.58}, "fail_below": 0.65, "low": ["general csf"], "passed": false}
}"##;

#[test]
fn a_table_the_pipeline_wrote_has_no_maker_and_is_written_and_read_as_it_was() {
    let table = read_table("regions_a.json", PIPELINE).expect("reads");
    assert_eq!(table.made, None);
    assert_eq!(table.entries.len(), 5);
    assert_eq!(round_trip(&table), table);
}

#[test]
fn the_text_is_the_shape_the_pipeline_writes() {
    let text = write_table(&head()).expect("writes");
    let value: Value = serde_json::from_str(&text).expect("json");
    assert_eq!(value["title"], "A made-up head");
    assert_eq!(value["model"], "SynthSeg 2.0, standard model");
    let hippocampus = &value["regions"]["hippocampus"];
    assert_eq!(hippocampus["group"], "Deep brain");
    assert_eq!(hippocampus["colour"], "#dcd814");
    assert_eq!(hippocampus["ids"], json!({"left": 17, "right": 53}));
    assert_eq!(value["regions"]["brain stem"]["ids"]["midline"], 16);
    let eyeball = &value["regions"]["eyeball"];
    assert_eq!(
        (&eyeball["approximate"], &eyeball["how"]),
        (&json!(true), &json!("Rules."))
    );
    assert_eq!(
        eyeball.get("colour"),
        None,
        "colours that are the numbers' own are left to be found"
    );
    let check = &value["qc"];
    assert_eq!(check["scores"], json!({"general csf": 0.58}));
    assert_eq!(check["fail_below"], 0.65);
    assert_eq!(check["passed"], false);
    assert_eq!(check["low"], json!(["general csf"]));
    // A table is listed in the order of its names.
    let at = |name: &str| text.find(&format!("\"{name}\"")).expect(name);
    assert!(at("hippocampus") < at("brain stem") && at("brain stem") < at("eyeball"));
    assert!(text.ends_with("}\n"), "one line end after the last brace");
}

#[test]
fn a_table_of_the_other_shapes_is_written_in_this_one_and_read_as_it_was() {
    let plain = r##"{"title": "Small", "parts": {"1": "cochlea", "2": "vestibule"}, "colours": {"1": "#E69F00"}}"##;
    let slicer = "# Color table file\n17 hippocampus_(left) 220 216 20 255\n53 hippocampus_(right) 220 216 20 255\n16 brain_stem 119 159 176 255\n";
    for (name, text) in [("a.json", plain), ("labels.ctbl", slicer)] {
        let table = read_table(name, text).expect("reads");
        assert_eq!(round_trip(&table), table, "{name}");
    }
    let bare = Table {
        title: String::new(),
        entries: vec![entry("thalamus", Side::Left, 10, Trust::NotSaid)],
        verdict: None,
        made: None,
    };
    let text = write_table(&bare).expect("writes");
    assert!(!text.contains("\"model\"") && !text.contains("\"voxa\"") && !text.contains("\"qc\""));
    assert_eq!(round_trip(&bare), bare);
}

/// The trust of each entry, in order, once the table has been written and read.
fn trusts_after_writing(table: &Table) -> Vec<Trust> {
    let read = round_trip(table);
    read.entries.into_iter().map(|entry| entry.trust).collect()
}

#[test]
fn trust_is_never_raised_in_the_writing() {
    let pair = |first: Trust, second: Trust| Table {
        entries: vec![
            entry("a", Side::Left, 1, first),
            entry("b", Side::Left, 2, second),
        ],
        made: None,
        ..head()
    };
    // Everything a model made: a model is said to have made it.
    let said = trusts_after_writing(&pair(Trust::Model, Trust::Model));
    assert_eq!(said, [Trust::Model, Trust::Model]);
    // One part not said to be: none is said to be.
    let said = trusts_after_writing(&pair(Trust::Model, Trust::NotSaid));
    assert_eq!(said, [Trust::NotSaid, Trust::NotSaid]);
    let said = trusts_after_writing(&pair(Trust::NotNamed, Trust::Model));
    assert_eq!(said, [Trust::NotSaid, Trust::NotSaid]);
    // The same name, a model's on one side and not said to be on the other.
    let one_name = Table {
        entries: vec![
            entry("a", Side::Left, 1, Trust::Model),
            entry("a", Side::Right, 2, Trust::NotSaid),
        ],
        made: None,
        ..head()
    };
    assert_eq!(
        trusts_after_writing(&one_name),
        [Trust::NotSaid, Trust::NotSaid]
    );
    // Approximate stays approximate, whatever the other side of the name is.
    let rule = Trust::Approximate("By rule.".to_owned());
    let one_name = Table {
        entries: vec![
            entry("a", Side::Left, 1, Trust::Model),
            entry("a", Side::Right, 2, rule.clone()),
        ],
        made: None,
        ..head()
    };
    assert_eq!(trusts_after_writing(&one_name), [rule.clone(), rule]);
}

#[test]
fn the_sides_of_a_name_are_brought_together_in_the_order_of_its_first_side() {
    let apart = Table {
        entries: vec![
            entry("a", Side::Left, 1, Trust::Model),
            entry("b", Side::Left, 2, Trust::Model),
            entry("a", Side::Right, 3, Trust::Model),
            entry("b", Side::Right, 4, Trust::Model),
        ],
        made: None,
        verdict: None,
        ..head()
    };
    let read = round_trip(&apart);
    let said: Vec<_> = read
        .entries
        .iter()
        .map(|e| (e.name.as_str(), e.side, e.ids[0]))
        .collect();
    let expected = [
        ("a", Side::Left, 1),
        ("a", Side::Right, 3),
        ("b", Side::Left, 2),
        ("b", Side::Right, 4),
    ];
    assert_eq!(said, expected);
}

#[test]
fn a_name_twice_on_one_side_is_refused_and_one_on_both_is_not() {
    let twice = |names: [&str; 2], sides: [Side; 2]| Table {
        entries: vec![
            entry(names[0], sides[0], 1, Trust::Model),
            entry(names[1], sides[1], 2, Trust::Model),
        ],
        ..head()
    };
    assert_eq!(
        write_table(&twice(["a", "a"], [Side::Left, Side::Left])),
        Err(WriteError::NameTwice)
    );
    assert_eq!(
        write_table(&twice(["a", "a"], [Side::Midline, Side::Midline])),
        Err(WriteError::NameTwice)
    );
    assert_eq!(
        write_table(&twice(["a", " a "], [Side::Right, Side::Right])),
        Err(WriteError::NameTwice),
        "a table keeps a name without the space round it"
    );
    let long = "n".repeat(80);
    assert_eq!(
        write_table(&twice(
            [&format!("{long}x"), &format!("{long}y")],
            [Side::Left; 2]
        )),
        Err(WriteError::NameTwice),
        "nor does it keep more than 80 characters of one"
    );
    assert!(write_table(&twice(["a", "a"], [Side::Left, Side::Right])).is_ok());
    assert!(write_table(&twice(["a", "b"], [Side::Left, Side::Left])).is_ok());
}

#[test]
fn names_with_marks_of_punctuation_and_letters_of_any_script_come_back() {
    let odd = [
        "He said \"hi\"",
        "back\\slash",
        "tab\there",
        "été 日本語",
        "line\nbreak",
        "{[:,]}",
    ];
    let entries = odd
        .iter()
        .zip(1..)
        .map(|(name, id)| entry(name, Side::Midline, id, Trust::NotSaid));
    let table = Table {
        title: "Quote \" and \\ in a title".to_owned(),
        entries: entries.collect(),
        verdict: None,
        made: None,
    };
    assert_eq!(round_trip(&table), table);
}

#[test]
fn what_is_kept_of_a_name_is_what_a_table_reading_keeps() {
    let long = "n".repeat(100);
    let table = Table {
        title: format!("  {long}  "),
        entries: vec![Entry {
            group: long.clone(),
            trust: Trust::Approximate(format!("{long}{long}{long}{long}")),
            ..entry(&long, Side::Left, 5, Trust::NotSaid)
        }],
        verdict: None,
        made: None,
    };
    let read = round_trip(&table);
    assert_eq!(read.title, "n".repeat(80));
    assert_eq!(read.entries[0].name, "n".repeat(80));
    assert_eq!(read.entries[0].group, "n".repeat(80));
    assert_eq!(read.entries[0].trust, Trust::Approximate("n".repeat(300)));
}

#[test]
fn a_name_with_no_number_names_nothing_and_one_with_several_is_written_with_its_first() {
    let table = Table {
        entries: vec![
            Entry {
                ids: Vec::new(),
                ..entry("none", Side::Left, 1, Trust::Model)
            },
            Entry {
                ids: vec![7, 8],
                ..entry("two", Side::Left, 1, Trust::Model)
            },
        ],
        ..head()
    };
    let read = round_trip(&table);
    assert_eq!(read.entries.len(), 1);
    assert_eq!(
        (read.entries[0].name.as_str(), &read.entries[0].ids),
        ("two", &vec![7])
    );
}

#[test]
fn a_table_longer_than_the_reader_takes_is_too_large() {
    let entries =
        (1..=60_000).map(|id| entry(&format!("part {id}"), Side::Midline, id, Trust::Model));
    let huge = Table {
        entries: entries.collect(),
        ..head()
    };
    assert_eq!(write_table(&huge), Err(WriteError::TooLarge));
}
