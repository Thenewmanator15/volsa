//! Series whose slices cannot be placed by one transform, though each file states a position.

use volsa_core::SeriesProblem;

use super::tests::{heights, indexed, stack};
use crate::testkit::{SliceOptions, classic_slice};

#[test]
fn index_flags_slices_that_face_different_ways() {
    // A locating series: one slice in each plane. Their positions happen to step evenly along
    // the first slice's normal, so the gaps alone would not give it away.
    let planes = [
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],  // across the body
        [0.0, 1.0, 0.0, 0.0, 0.0, -1.0], // from the side
        [1.0, 0.0, 0.0, 0.0, 0.0, -1.0], // from the front
    ];
    let files: Vec<(String, Vec<u8>)> = planes
        .iter()
        .zip(1..)
        .map(|(plane, number)| {
            let opts = SliceOptions {
                instance_number: number,
                orientation: Some(*plane),
                position: Some([-100.0, -120.0, f64::from(number) * 50.0]),
                ..SliceOptions::default()
            };
            (format!("locating/{number}"), classic_slice(&opts))
        })
        .collect();
    let index = indexed(files.clone());
    let series = &index.series[0];
    assert_eq!(series.info.problems, vec![SeriesProblem::MixedOrientations]);
    assert!(
        !series.info.has_geometry(),
        "no single transform places slices in different planes"
    );
    assert_eq!(
        heights(&index, series, &files),
        vec![1, 2, 3],
        "kept in the scanner's order"
    );
}

#[test]
fn index_accepts_directions_that_differ_only_by_rounding() {
    let base = SliceOptions::default();
    let mut files = stack(4, 5.0, &base);
    let nudged = SliceOptions {
        instance_number: 5,
        orientation: Some([1.0, 0.0, 0.0, 0.0, 0.999_999_9, 0.000_447]),
        position: Some([-100.0, -120.0, 20.0]),
        ..base
    };
    files.push(("IMAGES/S1/nudged".to_owned(), classic_slice(&nudged)));
    let index = indexed(files);
    assert_eq!(index.series[0].info.problems, Vec::new());
    assert!(index.series[0].info.has_geometry());
}

#[test]
fn index_treats_directions_that_are_not_directions_as_missing_geometry() {
    // Derived images sometimes carry zeros where the two directions should be.
    let zeros = SliceOptions {
        orientation: Some([0.0; 6]),
        ..SliceOptions::default()
    };
    let index = indexed(stack(4, 5.0, &zeros));
    assert_eq!(
        index.series[0].info.problems,
        vec![SeriesProblem::MissingGeometry]
    );
    assert!(!index.series[0].info.has_geometry());

    // Two directions that are not at right angles describe no image plane either.
    let skewed = SliceOptions {
        orientation: Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
        ..SliceOptions::default()
    };
    let index = indexed(stack(4, 5.0, &skewed));
    assert_eq!(
        index.series[0].info.problems,
        vec![SeriesProblem::MissingGeometry]
    );
}

#[test]
fn index_flags_a_series_compressed_in_a_way_volsa_cannot_read() {
    // The header reads normally; it is the pixels that could not be decoded.
    let jpeg_ls = SliceOptions {
        transfer_syntax: "1.2.840.10008.1.2.4.80".to_owned(),
        ..SliceOptions::default()
    };
    let index = indexed(stack(12, 5.0, &jpeg_ls));
    let problems = &index.series[0].info.problems;
    assert!(
        matches!(problems.as_slice(), [SeriesProblem::UnsupportedTransferSyntax(name)] if name.contains("JPEG-LS")),
        "{problems:?}"
    );
    assert!(
        index.series[0].info.has_geometry(),
        "where the slices are is still known"
    );
    assert_eq!(
        crate::best_series(&index),
        None,
        "never the series opened automatically"
    );
}
