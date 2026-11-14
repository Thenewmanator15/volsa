//! Reading a made-up segmentation on a worker into labels, with its names and colours.

use volsa_dicom::Index;
use volsa_dicom::testkit::{SegFrame, SegOptions, SegSegment, segmentation};
use volsa_labels::colour_for;

use super::super::tests::{Bench, folder_of};
use super::CUT_AT_HALF;
use crate::action::{Action, Effect};
use crate::labels::NamesFrom;

const SEG: &str = "SEG/S0001";

fn frame(z: f64, segment: u16, on: &[usize]) -> SegFrame {
    SegFrame {
        position: [-100.0, -120.0, z],
        segment,
        mask: (0..15).map(|at| u8::from(on.contains(&at))).collect(),
    }
}

/// Two segments over two slices: the first named and red, the second with neither a name nor
/// a colour.
fn two_segments() -> SegOptions {
    SegOptions {
        segments: vec![
            SegSegment {
                number: 1,
                label: "Part one".to_owned(),
                // CIELab of the sRGB red, as DICOM scales it.
                lab: Some([34_891, 53_477, 50_167]),
            },
            SegSegment {
                number: 2,
                label: String::new(),
                lab: None,
            },
        ],
        frames: vec![frame(0.0, 1, &[0, 1, 2]), frame(5.0, 2, &[4])],
        ..SegOptions::default()
    }
}

/// Index an export of one segmentation, and ask for it to be read.
fn read(bench: &Bench, bytes: Vec<u8>) -> (tempfile::TempDir, Index) {
    let folder = folder_of(&[(SEG.to_owned(), bytes)]);
    let index = bench
        .index(folder.path())
        .expect("an export of a segmentation is read");
    let asked = Effect::ReadSegmentation {
        entry: SEG.to_owned(),
    };
    bench.workers.run(asked, Some(&index));
    (folder, index)
}

#[test]
fn a_segmentation_is_read_on_a_worker_as_labels_with_its_names_and_colours() {
    let bench = Bench::with_budget(1 << 20);
    let (_folder, index) = read(&bench, segmentation(&two_segments()));
    assert_eq!(index.series.len(), 0, "a segmentation is not a scan");
    let Action::LabelsRead {
        title,
        parts,
        names_from,
        made,
        verdict,
        run,
        ..
    } = bench.next()
    else {
        panic!("labels are read");
    };
    assert_eq!(title, "MADE_UP");
    let names: Vec<_> = parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(names, ["Part one", "Segment 2"]);
    let voxels: Vec<_> = parts.iter().map(|part| part.voxels).collect();
    assert_eq!(voxels, [3, 1]);
    let [red, green, blue] = parts.first().expect("a first part").colour;
    assert!(
        red >= 253 && green <= 2 && blue <= 2,
        "{:?}",
        [red, green, blue]
    );
    assert_eq!(parts.get(1).map(|part| part.colour), Some(colour_for(2)));
    assert_eq!(
        (names_from, made, verdict, run),
        (NamesFrom::Table, None, None, None)
    );
    assert!(
        bench.workers.labels().is_some(),
        "the labels are kept to be drawn"
    );
}

#[test]
fn fractional_masks_are_said_to_be_cut_at_half() {
    let bench = Bench::with_budget(1 << 20);
    let opts = SegOptions {
        fractional: Some(255),
        frames: vec![SegFrame {
            mask: vec![255, 128, 127, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ..frame(0.0, 1, &[])
        }],
        ..two_segments()
    };
    let (_folder, _index) = read(&bench, segmentation(&opts));
    let Action::Post(notice) = bench.next() else {
        panic!("the cut is said first");
    };
    assert_eq!(notice.text, CUT_AT_HALF);
    let Action::LabelsRead { parts, .. } = bench.next() else {
        panic!("labels are read");
    };
    assert_eq!(parts.first().map(|part| part.voxels), Some(2));
}

#[test]
fn a_segmentation_that_cannot_be_read_fails_in_a_sentence_with_no_path() {
    let bench = Bench::with_budget(1 << 20);
    let opts = SegOptions {
        frames: vec![
            frame(0.0, 1, &[0]),
            frame(5.0, 1, &[0]),
            frame(12.0, 1, &[0]),
        ],
        ..two_segments()
    };
    let (folder, _index) = read(&bench, segmentation(&opts));
    let Action::LabelsFailed(why) = bench.next() else {
        panic!("the read fails");
    };
    assert!(why.contains("not evenly spaced"), "{why}");
    let path = folder.path().to_string_lossy().into_owned();
    assert!(!why.contains(&path) && !why.contains(SEG), "{why}");
    assert!(bench.workers.labels().is_none());
}
