//! Tables of names in the three shapes read, label sets built from them, and what is said
//! of a part in words.

use volsa_core::Affine;

use super::*;
use crate::nifti::LabelGrid;
use crate::set::{LabelSet, NOT_NAMED, SetError, label_set};
use crate::words::{trust_in_words, verdict_in_words, volume_in_words};

/// A table as the labelling pipeline writes it: names with sides, a group, a colour, how
/// each was made, and the model's own check.
const REGIONS: &str = r##"{
 "title": "A made-up head",
 "model": "robust",
 "regions": {
  "hippocampus": {"group": "Deep brain", "colour": "#dcd814", "ids": {"left": 17, "right": 53}, "ml": {"left": 1.0, "right": 1.0}},
  "brain stem": {"group": "Cerebellum and brainstem", "colour": "#779fb0", "ids": {"midline": 16}},
  "eyeball": {"group": "Outside the brain", "colour": "#3cb4e6", "ids": {"left": 101, "right": 102}, "approximate": true, "how": "Rules."}
 },
 "qc": {"scores": {"general white matter": 0.8, "general csf": 0.58}, "fail_below": 0.65, "low": ["general csf"], "passed": false}
}"##;

fn regions() -> Table {
    read_table("regions_a.json", REGIONS).expect("reads")
}

/// A grid of six voxels, each 1 by 2 by 3 mm, holding these numbers.
fn grid(ids: &[u32]) -> LabelGrid {
    let count = u32::try_from(ids.len()).expect("a few");
    LabelGrid {
        dims: [count, 1, 1],
        ids: ids.to_vec(),
        placement: Affine::from_columns(
            [1.0, 0.0, 0.0],
            [0.0, 2.0, 0.0],
            [0.0, 0.0, 3.0],
            [0.0; 3],
        ),
    }
}

fn names(set: &LabelSet) -> Vec<(&str, Side)> {
    let parts = set.parts.iter();
    parts.map(|part| (part.name.as_str(), part.side)).collect()
}

#[test]
fn the_pipelines_table_gives_each_name_its_sides_group_colour_and_trust() {
    let table = regions();
    assert_eq!(table.title, "A made-up head");
    let said: Vec<_> = table
        .entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.side, entry.ids.as_slice()))
        .collect();
    let expected: [(&str, Side, &[u32]); 5] = [
        ("hippocampus", Side::Left, &[17]),
        ("hippocampus", Side::Right, &[53]),
        ("brain stem", Side::Midline, &[16]),
        ("eyeball", Side::Left, &[101]),
        ("eyeball", Side::Right, &[102]),
    ];
    assert_eq!(
        said, expected,
        "in the order of the file, left before right"
    );
    let first = &table.entries[0];
    assert_eq!(
        (first.group.as_str(), first.colour),
        ("Deep brain", [0xdc, 0xd8, 0x14])
    );
    assert_eq!(first.trust, Trust::Model);
    assert_eq!(
        table.entries[3].trust,
        Trust::Approximate("Rules.".to_owned())
    );
    let verdict = table.verdict.expect("the model's own check");
    assert!(!verdict.passed);
    assert_eq!(verdict.fail_below, 0.65);
    assert_eq!(verdict.low, [("general csf".to_owned(), 0.58)]);
}

#[test]
fn a_table_that_says_less_is_read_for_what_it_says() {
    let bare = r#"{"regions": {"thalamus": {"ids": {"left": 10}}, "odd": {"ids": {}}, "pons": {"ids": {"midline": 174}, "approximate": true}}}"#;
    let table = read_table("regions.json", bare).expect("reads");
    assert_eq!(table.title, "");
    assert_eq!(table.verdict, None);
    assert_eq!(
        table.entries.len(),
        2,
        "a name with no numbers names nothing"
    );
    let thalamus = &table.entries[0];
    assert_eq!((thalamus.side, thalamus.group.as_str()), (Side::Left, ""));
    assert_eq!(
        thalamus.trust,
        Trust::NotSaid,
        "the file does not say a model made it"
    );
    assert_eq!(table.entries[1].trust, Trust::Approximate(String::new()));
    assert_ne!(
        thalamus.colour, table.entries[1].colour,
        "colours are found for them"
    );
}

#[test]
fn a_plain_table_of_numbers_and_names_is_read() {
    let plain = r##"{"parts": {"1": "cochlea", "2": "vestibule", "x": "not a number"}, "colours": {"1": "#E69F00"}}"##;
    let table = read_table("anything.json", plain).expect("reads");
    let said: Vec<_> = table
        .entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.ids.clone()))
        .collect();
    assert_eq!(said, [("cochlea", vec![1]), ("vestibule", vec![2])]);
    assert_eq!(table.entries[0].colour, [0xE6, 0x9F, 0x00]);
    assert!(
        table
            .entries
            .iter()
            .all(|entry| entry.side == Side::Midline)
    );
    assert!(
        table
            .entries
            .iter()
            .all(|entry| entry.trust == Trust::NotSaid)
    );
}

#[test]
fn a_slicer_colour_table_is_read_by_its_name() {
    let text = "# Color table file\n0 background 0 0 0 0\n17 hippocampus_(left) 220 216 20 255\n\n53 hippocampus_(right) 220 216 20\n16 brain_stem 119 159 176 255\n9 short 1\nx y 1 2 3 4\n";
    let table = read_table("labels.CTBL", text).expect("reads");
    let said: Vec<_> = table
        .entries
        .iter()
        .map(|entry| {
            (
                entry.name.as_str(),
                entry.side,
                entry.ids.clone(),
                entry.colour,
            )
        })
        .collect();
    let expected = [
        ("hippocampus", Side::Left, vec![17], [220, 216, 20]),
        ("hippocampus", Side::Right, vec![53], [220, 216, 20]),
        ("brain stem", Side::Midline, vec![16], [119, 159, 176]),
    ];
    assert_eq!(
        said, expected,
        "the background and the lines that are short or odd are passed over"
    );
    assert!(
        table
            .entries
            .iter()
            .all(|entry| entry.trust == Trust::NotSaid)
    );
}

#[test]
fn what_is_not_a_table_is_refused_and_names_are_kept_to_a_length() {
    assert_eq!(
        read_table("a.json", "not json at all"),
        Err(TableError::NotATable)
    );
    assert_eq!(
        read_table("a.json", "[1, 2, 3]"),
        Err(TableError::NotATable)
    );
    assert_eq!(
        read_table("a.json", r#"{"something": "else"}"#),
        Err(TableError::NotATable)
    );
    assert_eq!(
        read_table("a.ctbl", "nothing here\n"),
        Err(TableError::NotATable)
    );
    let long = " ".repeat(5 << 20);
    assert_eq!(read_table("a.json", &long), Err(TableError::TooLong));
    let wordy = format!(r#"{{"parts": {{"1": "{}"}}}}"#, "n".repeat(500));
    let table = read_table("a.json", &wordy).expect("reads");
    assert_eq!(table.entries[0].name.chars().count(), MAX_NAME);
    for refusal in [TableError::NotATable, TableError::TooLong] {
        assert!(refusal.to_string().ends_with('.'), "{refusal}");
    }
}

#[test]
fn a_label_set_has_a_part_for_everything_in_the_volume_and_nothing_else() {
    let set = label_set(&grid(&[0, 16, 17, 17, 53, 99]), Some(&regions())).expect("fits");
    let expected = [
        ("hippocampus", Side::Left),
        ("hippocampus", Side::Right),
        ("brain stem", Side::Midline),
        ("Label 99", Side::Midline),
    ];
    assert_eq!(names(&set), expected, "no eyeball: it is not in the volume");
    assert_eq!(set.numbers, [0, 3, 1, 1, 2, 4]);
    let left = &set.parts[0];
    assert_eq!(left.voxels, 2);
    assert!(
        (left.ml - 0.012).abs() < 1.0e-12,
        "two voxels of 6 cubic millimetres: {}",
        left.ml
    );
    let unnamed = &set.parts[3];
    assert_eq!(
        (unnamed.group.as_str(), &unnamed.trust),
        (NOT_NAMED, &Trust::NotNamed)
    );
    assert_eq!(set.title, "A made-up head");
    assert!(set.verdict.is_some());
    assert_eq!(set.dims, [6, 1, 1]);
}

#[test]
fn with_no_table_every_number_is_a_part_with_a_colour_of_its_own() {
    let numbers = grid(&[0, 16, 17, 17, 53, 99]);
    let set = label_set(&numbers, None).expect("fits");
    let said: Vec<&str> = set.parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(said, ["Label 16", "Label 17", "Label 53", "Label 99"]);
    let mut colours: Vec<[u8; 3]> = set.parts.iter().map(|part| part.colour).collect();
    let again = label_set(&numbers, None).expect("fits");
    let same: Vec<[u8; 3]> = again.parts.iter().map(|part| part.colour).collect();
    assert_eq!(colours, same, "the same colours every time");
    colours.sort_unstable();
    colours.dedup();
    assert_eq!(colours.len(), 4, "and no two alike");
    assert!(set.parts.iter().all(|part| part.trust == Trust::NotNamed));
    assert_eq!((set.title.as_str(), &set.verdict), ("", &None));
}

#[test]
fn a_number_named_twice_belongs_to_the_first_name_and_too_many_numbers_are_refused() {
    let twice = r#"{"parts": {"5": "first", "5 ": "second"}}"#;
    let table = read_table("a.json", twice).expect("reads");
    let set = label_set(&grid(&[5, 5, 0]), Some(&table)).expect("fits");
    assert_eq!(names(&set), [("first", Side::Midline)]);
    let many: Vec<u32> = (0..70_000).collect();
    assert_eq!(
        label_set(&grid(&many), None).err(),
        Some(SetError::TooManyParts)
    );
    assert!(SetError::TooManyParts.to_string().ends_with('.'));
}

#[test]
fn the_models_own_check_is_said_in_a_sentence() {
    let failed = verdict_in_words(&regions().verdict.expect("there is one"));
    assert!(
        failed.starts_with("The labelling model's own check failed"),
        "{failed}"
    );
    assert!(failed.contains("general csf") && failed.contains("0.58") && failed.contains("0.65"));
    assert!(
        !failed.contains("general white matter"),
        "what passed is not listed: {failed}"
    );
    let passed = Verdict {
        passed: true,
        fail_below: 0.65,
        low: Vec::new(),
    };
    let said = verdict_in_words(&passed);
    assert!(
        said.contains("passed") && said.contains("model's opinion of its own work"),
        "{said}"
    );
}

#[test]
fn volumes_and_trust_are_said_in_few_words() {
    assert_eq!(volume_in_words(4.04), "4.0 ml");
    assert_eq!(volume_in_words(1.0), "1.0 ml");
    assert_eq!(volume_in_words(0.0478), "48 mm³");
    assert_eq!(volume_in_words(0.0), "0 mm³");
    assert_eq!(volume_in_words(f64::NAN), "");
    let said = [
        Trust::Model,
        Trust::Approximate("Rules.".to_owned()),
        Trust::NotNamed,
        Trust::NotSaid,
    ]
    .map(|trust| trust_in_words(&trust));
    assert_eq!(said, ["model", "approximate", "not named", "not said"]);
}
