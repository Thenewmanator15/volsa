//! Whatever a file holds, reading it as labels or as a table of names gives an answer or a
//! reason, and never stops the program.

use proptest::collection::vec;
use proptest::prelude::*;

use crate::testkit::{NiftiOptions, gzipped, nifti};
use crate::{label_set, read_labels, read_table};

/// A sound label file: 3 by 4 by 5 voxels of 2 mm, numbered from 0 to 7 over and over.
fn sound() -> Vec<u8> {
    nifti(&NiftiOptions {
        dims: [3, 4, 5],
        ids: (0..60).map(|place| place % 8).collect(),
        kind: 512,
        srow: Some([
            [2.0, 0.0, 0.0, -3.0],
            [0.0, 2.0, 0.0, -4.0],
            [0.0, 0.0, 2.0, -5.0],
        ]),
        quatern: None,
        big_endian: false,
        slope: 1.0,
    })
}

/// A sound table in the shape a labelling pipeline writes.
const TABLE: &str = r##"{"title": "Made up", "model": "made-up", "regions": {
  "one": {"group": "A", "colour": "#102030", "ids": {"left": 1, "right": 2}},
  "two": {"group": "B", "ids": {"midline": 3}, "approximate": true, "how": "By rule."}},
  "qc": {"scores": {"one": 0.5}, "fail_below": 0.65, "passed": false}}"##;

/// The characters a table is damaged with: those its own punctuation is made of, and a few
/// others.
const MARKS: &[u8] = b"{}[]\":,#-.0159ae \n\\";

proptest! {
    #[test]
    fn any_bytes_are_labels_or_a_reason(bytes in vec(any::<u8>(), 0..1024)) {
        let _ = read_labels(&bytes);
        let _ = read_labels(&gzipped(&bytes));
    }

    #[test]
    fn a_sound_file_damaged_in_its_header_or_cut_short_is_labels_or_a_reason(
        changes in vec((0_usize..352, any::<u8>()), 1..12),
        kept in 0_usize..600,
        zipped in any::<bool>(),
    ) {
        let mut file = sound();
        for (at, byte) in changes {
            file[at] = byte;
        }
        file.truncate(kept);
        let file = if zipped { gzipped(&file) } else { file };
        if let Ok(grid) = read_labels(&file) {
            // Whatever numbers it holds can be listed, with a table or without.
            let table = read_table("regions.json", TABLE).expect("a sound table");
            let _ = label_set(&grid, Some(&table));
            let _ = label_set(&grid, None);
        }
    }

    #[test]
    fn a_sound_table_damaged_anywhere_is_a_table_or_a_reason(
        changes in vec((0_usize..TABLE.len(), 0_usize..MARKS.len()), 1..10),
        slicer in any::<bool>(),
    ) {
        let mut text = TABLE.as_bytes().to_vec();
        for (at, mark) in changes {
            text[at] = MARKS[mark];
        }
        let text = String::from_utf8(text).expect("the table and the marks are ASCII");
        let name = if slicer { "labels.ctbl" } else { "regions.json" };
        if let Ok(table) = read_table(name, &text) {
            let grid = read_labels(&sound()).expect("a sound file");
            let _ = label_set(&grid, Some(&table));
        }
    }

    #[test]
    fn any_text_is_a_table_or_a_reason(text in ".{0,300}", slicer in any::<bool>()) {
        let name = if slicer { "labels.ctbl" } else { "regions.json" };
        let _ = read_table(name, &text);
    }
}
