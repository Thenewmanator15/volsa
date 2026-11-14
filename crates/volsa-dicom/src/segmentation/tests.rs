//! Reading made-up segmentations: masks of each kind into the right voxels, names and
//! colours, how they are listed, and what is refused.

use super::colour::lab_to_srgb;
use super::*;
use crate::index::index;
use crate::testkit::{
    MemorySource, SegFrame, SegOptions, SegSegment, SliceOptions, classic_slice, segmentation,
};

/// 5 × 3 frames: 15 pixels, which do not fill whole bytes.
pub(super) const PIXELS: usize = 15;

/// A mask of 15 pixels with these on.
pub(super) fn mask(on: &[usize]) -> Vec<u8> {
    (0..PIXELS).map(|at| u8::from(on.contains(&at))).collect()
}

pub(super) fn frame(z: f64, segment: u16, on: &[usize]) -> SegFrame {
    SegFrame {
        position: [-10.0, -20.0, z],
        segment,
        mask: mask(on),
    }
}

fn named(number: u16, label: &str) -> SegSegment {
    SegSegment {
        number,
        label: label.to_owned(),
        lab: None,
    }
}

/// Two segments in three frames over two slices 2.5 mm apart, the second segment over the
/// first at pixel 7.
fn two_segments() -> SegOptions {
    SegOptions {
        segments: vec![named(1, "Part one"), named(2, "Part two")],
        frames: vec![
            frame(0.0, 1, &[0, 1, 7, 14]),
            frame(0.0, 2, &[7, 8, 9]),
            frame(2.5, 1, &[3, 4, 13]),
        ],
        ..SegOptions::default()
    }
}

fn read_ok(opts: &SegOptions) -> Segmentation {
    read_segmentation(&segmentation(opts)).expect("the made-up segmentation is read")
}

fn refusal(bytes: &[u8]) -> String {
    read_segmentation(bytes).expect_err("refused").0
}

/// The voxels that hold `number`, counted columns fastest.
fn holding(numbers: &[u16], number: u16) -> Vec<usize> {
    let at = numbers.iter().enumerate();
    at.filter(|(_, held)| **held == number)
        .map(|(at, _)| at)
        .collect()
}

#[test]
fn binary_masks_that_do_not_fill_whole_bytes_land_on_their_voxels() {
    let read = read_ok(&two_segments());
    assert_eq!(read.dims, [5, 3, 2]);
    assert_eq!(holding(&read.numbers, 1), vec![0, 1, 14, 18, 19, 28]);
    // Pixel 7 is in both segments; the larger number is kept.
    assert_eq!(holding(&read.numbers, 2), vec![7, 8, 9]);
    assert!(!read.fractional);
}

#[test]
fn segments_keep_their_names_order_and_titles() {
    let opts = SegOptions {
        referenced_series: Some("1.2.826.0.1.3680043.9.9999.1.1".to_owned()),
        ..two_segments()
    };
    let read = read_ok(&opts);
    let labels: Vec<_> = read
        .segments
        .iter()
        .map(|s| (s.number, s.label.as_str()))
        .collect();
    assert_eq!(labels, vec![(1, "Part one"), (2, "Part two")]);
    assert_eq!(read.title, "MADE_UP");
    let series = read.referenced_series.as_deref();
    assert_eq!(series, Some("1.2.826.0.1.3680043.9.9999.1.1"));
}

#[test]
fn a_recommended_colour_is_turned_from_cielab_into_srgb() {
    // CIELab of the sRGB red, as DICOM scales it: L* 53.24, a* 80.09, b* 67.20.
    let red_lab = [34_891, 53_477, 50_167];
    let mut opts = two_segments();
    if let Some(first) = opts.segments.first_mut() {
        first.lab = Some(red_lab);
    }
    let read = read_ok(&opts);
    let colour = read
        .segments
        .first()
        .and_then(|s| s.colour)
        .expect("a colour");
    let near = |got: u8, want: u8| got.abs_diff(want) <= 2;
    assert!(
        near(colour[0], 255) && near(colour[1], 0) && near(colour[2], 0),
        "{colour:?}"
    );
    assert_eq!(read.segments.get(1).and_then(|s| s.colour), None);
    assert_eq!(lab_to_srgb(65_535, 32_896, 32_896), [255, 255, 255]);
    assert_eq!(lab_to_srgb(0, 32_896, 32_896), [0, 0, 0]);
}

#[test]
fn fractional_masks_are_cut_at_half_their_most() {
    let values = [0, 127, 128, 200, 255, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let opts = SegOptions {
        segments: vec![named(3, "Soft")],
        frames: vec![SegFrame {
            mask: values.to_vec(),
            ..frame(0.0, 3, &[])
        }],
        fractional: Some(255),
        ..SegOptions::default()
    };
    let read = read_ok(&opts);
    assert!(read.fractional);
    assert_eq!(read.dims, [5, 3, 1]);
    assert_eq!(holding(&read.numbers, 3), vec![2, 3, 4]);
}

#[test]
fn compressed_masks_are_refused_in_a_sentence() {
    let opts = SegOptions {
        transfer_syntax: "1.2.840.10008.1.2.4.50".to_owned(),
        ..two_segments()
    };
    let why = refusal(&segmentation(&opts));
    assert!(
        why.starts_with("This segmentation is stored compressed ("),
        "{why}"
    );
    assert!(why.ends_with("which Volsa does not read."), "{why}");
}

#[test]
fn masks_cut_short_are_refused() {
    let opts = SegOptions {
        drop_bytes: 2,
        ..two_segments()
    };
    let why = refusal(&segmentation(&opts));
    assert!(why.contains("less than its frames need"), "{why}");
}

#[test]
fn damaged_or_truncated_files_are_refused_without_a_panic() {
    let whole = segmentation(&two_segments());
    for cut in [0, 10, 132, 200, whole.len() / 2, whole.len() - 3] {
        let part = whole.get(..cut).expect("within the file");
        assert!(read_segmentation(part).is_err(), "cut at {cut}");
    }
    for at in (132..whole.len()).step_by(7) {
        let mut damaged = whole.clone();
        if let Some(byte) = damaged.get_mut(at) {
            *byte ^= 0xA5;
        }
        // Whatever it gives, it must give it rather than stop the program.
        let _ = read_segmentation(&damaged);
    }
}

#[test]
fn a_file_that_is_not_a_segmentation_is_refused() {
    let why = refusal(&classic_slice(&SliceOptions::default()));
    assert_eq!(why, "This file is not a segmentation that Volsa can read.");
    assert_eq!(refusal(b"not dicom at all"), DAMAGED);
}

#[test]
fn a_segmentation_is_listed_as_a_document_and_never_as_a_scan() {
    let files = vec![
        (
            "IMAGES/I0001".to_owned(),
            classic_slice(&SliceOptions::default()),
        ),
        ("SEG/S0001".to_owned(), segmentation(&two_segments())),
    ];
    let found = index(&mut MemorySource::new(files), &mut |_| {}).expect("indexed");
    assert_eq!(found.series.len(), 1, "only the scan is a series");
    let listed = found.documents.first().expect("the segmentation is listed");
    assert_eq!(listed.entry, "SEG/S0001");
    assert_eq!(listed.kind, crate::DocumentKind::Segmentation);
    assert_eq!(listed.title, "MADE_UP");
    assert!(found.skipped.is_empty(), "{:?}", found.skipped);
}
