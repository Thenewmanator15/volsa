//! The table of a run, made from `SynthSeg`'s own lists, is the table a file gives back: written
//! and read again it is the same, whichever model ran and whatever the scores said.
//!
//! The order of the labels, and which of them are the cortex's, are those of the golden
//! group's `lists.json`. The names and the sides are `SynthSeg`'s own, as its data files give
//! them and the converter writes them down; `lists.json` does not hold them.

use volsa_labels::{Made, Table, Verdict, read_table, write_table};
use volsa_models::{Lists, ModelSide, Structure, Wanted};

use super::tests::synthsegs_lists;
use crate::table;

use ModelSide::{Left, Midline, Right};

/// The name and side of each structure, by label, as `SynthSeg` names them.
const STRUCTURES: [(u32, &str, ModelSide); 33] = [
    (0, "background", Midline),
    (2, "cerebral white matter", Left),
    (3, "cerebral cortex", Left),
    (4, "lateral ventricle", Left),
    (5, "inferior lateral ventricle", Left),
    (7, "cerebellum white matter", Left),
    (8, "cerebellum cortex", Left),
    (10, "thalamus", Left),
    (11, "caudate", Left),
    (12, "putamen", Left),
    (13, "pallidum", Left),
    (14, "3rd ventricle", Midline),
    (15, "4th ventricle", Midline),
    (16, "brain-stem", Midline),
    (17, "hippocampus", Left),
    (18, "amygdala", Left),
    (24, "csf", Midline),
    (26, "accumbens area", Left),
    (28, "ventral DC", Left),
    (41, "cerebral white matter", Right),
    (42, "cerebral cortex", Right),
    (43, "lateral ventricle", Right),
    (44, "inferior lateral ventricle", Right),
    (46, "cerebellum white matter", Right),
    (47, "cerebellum cortex", Right),
    (49, "thalamus", Right),
    (50, "caudate", Right),
    (51, "putamen", Right),
    (52, "pallidum", Right),
    (53, "hippocampus", Right),
    (54, "amygdala", Right),
    (58, "accumbens area", Right),
    (60, "ventral DC", Right),
];

/// The areas of the cortex in the order of the lists, 1001 and up for the left and 2001 and up
/// for the right, which have the same names.
const AREAS: [&str; 34] = [
    "bankssts",
    "caudalanteriorcingulate",
    "caudalmiddlefrontal",
    "cuneus",
    "entorhinal",
    "fusiform",
    "inferiorparietal",
    "inferiortemporal",
    "isthmuscingulate",
    "lateraloccipital",
    "lateralorbitofrontal",
    "lingual",
    "medialorbitofrontal",
    "middletemporal",
    "parahippocampal",
    "paracentral",
    "parsopercularis",
    "parsorbitalis",
    "parstriangularis",
    "pericalcarine",
    "postcentral",
    "posteriorcingulate",
    "precentral",
    "precuneus",
    "rostralanteriorcingulate",
    "rostralmiddlefrontal",
    "superiorfrontal",
    "superiorparietal",
    "superiortemporal",
    "supramarginal",
    "frontalpole",
    "temporalpole",
    "transversetemporal",
    "insula",
];

/// The groups the scores network scores, in the order of its classes.
const GROUPS: [&str; 8] = [
    "general white matter",
    "general grey matter",
    "general csf",
    "cerebellum",
    "brainstem",
    "thalamus",
    "putamen+pallidum",
    "hippocampus+amygdala",
];

/// `SynthSeg`'s lists: the labels in their order from the golden group, with the names.
fn real_lists() -> Lists {
    let mut lists = synthsegs_lists();
    assert_eq!(lists.structures.len(), STRUCTURES.len());
    for (part, (label, name, side)) in lists.structures.iter_mut().zip(STRUCTURES) {
        assert_eq!(part.label, label, "the golden lists are in the order above");
        part.name = name.to_owned();
        part.side = side;
    }
    assert_eq!(lists.areas.len(), 1 + 2 * AREAS.len());
    let (nothing, areas) = lists.areas.split_at_mut(1);
    assert!(nothing.iter().all(|part| part.label == 0));
    let (left, right) = areas.split_at_mut(AREAS.len());
    for (part, name) in left
        .iter_mut()
        .chain(right.iter_mut())
        .zip(AREAS.iter().cycle())
    {
        part.name = (*name).to_owned();
        part.side = if part.label < 2000 { Left } else { Right };
    }
    assert_eq!(left.first().map(|part| part.label), Some(1001));
    assert_eq!(right.first().map(|part| part.label), Some(2001));
    lists.score_groups = GROUPS.map(str::to_owned).to_vec();
    lists
}

/// Who made the labels of a made-up run.
fn made() -> Made {
    Made {
        by: "Volsa 9.8.7".to_owned(),
        model: "SynthSeg 2.0, standard model".to_owned(),
        model_sum: "0123456789abcdef".repeat(4),
        device: "the processor".to_owned(),
        when: "2099-01-02 03:04".to_owned(),
        notes: vec!["The slices of this scan are 6 mm apart.".to_owned()],
    }
}

/// Whether two verdicts are the same. A score that is no number is the same as another.
fn same_verdict(a: &Verdict, b: &Verdict) -> bool {
    let same_score = |a: f64, b: f64| a == b || (a.is_nan() && b.is_nan());
    let same_low = a.low.len() == b.low.len()
        && (a.low.iter().zip(&b.low)).all(|(a, b)| a.0 == b.0 && same_score(a.1, b.1));
    a.passed == b.passed && a.fail_below == b.fail_below && same_low
}

/// Whether two tables are the same in every part, a score that is no number being the same
/// as another.
fn same_table(a: &Table, b: &Table) -> bool {
    let verdicts = match (&a.verdict, &b.verdict) {
        (Some(a), Some(b)) => same_verdict(a, b),
        (None, None) => true,
        _ => false,
    };
    verdicts && a.title == b.title && a.entries == b.entries && a.made == b.made
}

#[test]
fn the_real_lists_have_no_name_twice_on_one_side() {
    let lists = real_lists();
    let table = table(
        &lists,
        Wanted {
            robust: false,
            cortex: false,
        },
        &[],
    );
    let mut seen = std::collections::HashSet::new();
    for entry in &table.entries {
        let each = (entry.name.clone(), format!("{:?}", entry.side));
        assert!(
            seen.insert(each),
            "{} is named twice on one side",
            entry.name
        );
    }
    assert_eq!(table.entries.len(), 32 + 68);
}

#[test]
fn a_table_made_from_synthsegs_lists_is_the_table_a_file_gives_back() {
    let lists = real_lists();
    let passing: &[f32] = &[0.9, 0.8, 0.7, 0.95, 0.66, 0.65, 0.99, 0.7];
    let failing: &[f32] = &[0.9, 0.5, 0.7, 0.95, 0.66, 0.2, 0.99, 0.7];
    // The last three groups were not scored.
    let unscored: &[f32] = &[0.9, 0.8, 0.7, 0.95, 0.66];
    let runs = [
        ("passing", passing),
        ("failing", failing),
        ("unscored", unscored),
    ];
    for robust in [false, true] {
        for cortex in [false, true] {
            for (what, scores) in runs {
                let mut table = table(&lists, Wanted { robust, cortex }, scores);
                table.made = Some(Made {
                    model: table.title.clone(),
                    ..made()
                });
                let text = write_table(&table).expect("it can be written");
                let read = read_table("regions.json", &text).expect("it can be read");
                let which = format!("{what}, robust {robust}, cortex {cortex}");
                assert!(same_table(&read, &table), "{which}");
                let verdict = read.verdict.as_ref().expect("a verdict");
                match what {
                    "passing" => assert!(verdict.passed && verdict.low.is_empty(), "{which}"),
                    "failing" => assert_eq!(verdict.low.len(), 2, "{which}"),
                    _ => {
                        assert_eq!(verdict.low.len(), 3, "{which}");
                        assert!(verdict.low.iter().all(|(_, score)| score.is_nan()));
                    }
                }
            }
        }
    }
}

#[test]
fn the_pairs_of_the_table_lie_together_and_a_pair_has_one_colour() {
    let wanted = Wanted {
        robust: false,
        cortex: true,
    };
    let table = table(&real_lists(), wanted, &[]);
    let first = table.entries.iter().take(6);
    let said: Vec<_> = first
        .map(|entry| (entry.name.as_str(), entry.ids[0]))
        .collect();
    let expected = [
        ("cerebral white matter", 2),
        ("cerebral white matter", 41),
        ("cerebral cortex", 3),
        ("cerebral cortex", 42),
        ("lateral ventricle", 4),
        ("lateral ventricle", 43),
    ];
    assert_eq!(
        said, expected,
        "the lists give every left structure before any right one"
    );
    // A name's entries lie together, and each has the colour of the one that comes first.
    let mut colour_of = std::collections::HashMap::new();
    let mut last = "";
    for entry in &table.entries {
        let first = *colour_of.entry(entry.name.as_str()).or_insert_with(|| {
            assert_ne!(last, entry.name, "together");
            volsa_labels::colour_for(entry.ids[0])
        });
        assert_eq!(entry.colour, first, "{}", entry.name);
        last = entry.name.as_str();
    }
}

#[test]
fn names_with_a_space_at_either_end_or_at_the_cut_come_back_the_same() {
    let mut lists = super::tests::lists(&[(0, 0), (3, 1), (42, 2), (16, 3)], &[0, 1035], [3, 42]);
    lists.structures[1].name = "  cortex  ".to_owned();
    lists.structures[2].name = "cortex".to_owned();
    lists.structures[1].side = Left;
    lists.structures[2].side = Right;
    // A name listed first for its midline, and later for its left: left comes first.
    lists.structures[3].name = "stem".to_owned();
    let part = |label, name: String, side| Structure {
        label,
        name,
        side,
        class: 0,
    };
    lists.structures.push(part(14, "stem".to_owned(), Left));
    let cut = format!("{}  z", "b".repeat(79));
    lists.structures.push(part(18, cut, Midline));
    lists.areas[1].name = format!(" {} ", "c".repeat(100));
    lists.score_groups = vec![" general ".to_owned(), format!("{}  z", "d".repeat(79))];
    let wanted = Wanted {
        robust: false,
        cortex: true,
    };
    let mut table = table(&lists, wanted, &[0.9]);
    let names: Vec<&str> = table
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "cortex",
            "cortex",
            "stem",
            "stem",
            "b".repeat(79).as_str(),
            "c".repeat(80).as_str()
        ]
    );
    table.made = Some(made());
    let text = write_table(&table).expect("it can be written");
    let read = read_table("regions.json", &text).expect("it can be read");
    assert!(same_table(&read, &table));
    let sides: Vec<_> = read
        .entries
        .iter()
        .skip(2)
        .take(2)
        .map(|entry| entry.side)
        .collect();
    assert_eq!(
        sides,
        [volsa_labels::Side::Left, volsa_labels::Side::Midline],
        "the left of a name comes before its midline"
    );
    let low = &read.verdict.as_ref().expect("a verdict").low;
    assert_eq!(low.len(), 1, "the group that was not scored");
    assert_eq!(low[0].0, "d".repeat(79));
}
