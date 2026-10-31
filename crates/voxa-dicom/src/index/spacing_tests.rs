//! What the index says about a series' pixel spacing, rescale type and polarity.

use voxa_core::SeriesInfo;

use super::tests::{indexed, stack};
use crate::testkit::{SliceOptions, classic_slice};

/// The one series of a made-up stack of three slices built from `base`.
fn series_of(base: &SliceOptions) -> SeriesInfo {
    let index = indexed(stack(3, 5.0, base));
    index.series[0].info.clone()
}

#[test]
fn a_series_without_pixel_spacing_says_its_spacing_was_not_given() {
    let info = series_of(&SliceOptions {
        pixel_spacing: None,
        ..SliceOptions::default()
    });
    assert!(!info.spacing_given);
    assert_eq!(
        info.spacing_mm[..2],
        [1.0, 1.0],
        "the fallback for drawing is unchanged"
    );
}

#[test]
fn a_detectors_spacing_alone_is_not_taken_as_the_bodys() {
    let info = series_of(&SliceOptions {
        pixel_spacing: None,
        imager_pixel_spacing: Some([0.2, 0.2]),
        ..SliceOptions::default()
    });
    assert!(!info.spacing_given);
    assert_eq!(info.spacing_mm[..2], [1.0, 1.0]);

    let both = series_of(&SliceOptions {
        imager_pixel_spacing: Some([0.2, 0.2]),
        ..SliceOptions::default()
    });
    assert!(both.spacing_given);
    assert_eq!(
        both.spacing_mm[..2],
        [0.9, 0.9],
        "the body's spacing wins over the detector's"
    );
}

#[test]
fn a_series_with_pixel_spacing_says_so() {
    let info = series_of(&SliceOptions::default());
    assert!(info.spacing_given);
    assert_eq!(info.spacing_mm[..2], [0.9, 0.9]);
}

/// The first of two slices of one series, by name, is `first`; the second is `second`.
fn series_of_two(first: &SliceOptions, second: &SliceOptions) -> SeriesInfo {
    let slice = |name: &str, base: &SliceOptions, at: f64| {
        let opts = SliceOptions {
            position: Some([-100.0, -120.0, at]),
            ..base.clone()
        };
        (name.to_owned(), classic_slice(&opts))
    };
    let files = vec![slice("a", first, 0.0), slice("b", second, 5.0)];
    indexed(files).series[0].info.clone()
}

#[test]
fn a_series_whose_first_image_is_monochrome1_says_it_is_inverted() {
    let bright_is_dark = SliceOptions {
        monochrome1: true,
        ..SliceOptions::default()
    };
    assert!(series_of(&bright_is_dark).inverted);
    assert!(!series_of(&SliceOptions::default()).inverted);

    let plain = SliceOptions::default();
    assert!(series_of_two(&bright_is_dark, &plain).inverted);
    assert!(
        !series_of_two(&plain, &bright_is_dark).inverted,
        "it is the first image that says"
    );
}

#[test]
fn the_rescale_type_is_read_and_trimmed() {
    let named = series_of(&SliceOptions {
        rescale_type: Some("HU ".to_owned()),
        ..SliceOptions::default()
    });
    assert_eq!(named.rescale_type.as_deref(), Some("HU"));
    assert_eq!(series_of(&SliceOptions::default()).rescale_type, None);
}

/// The one series of `count` slices with no position in space, each `thickness` thick if any.
fn unplaced(count: i32, thickness: Option<f64>) -> SeriesInfo {
    let files = (0..count).map(|at| {
        let opts = SliceOptions {
            instance_number: at + 1,
            position: None,
            orientation: None,
            slice_thickness: thickness,
            ..SliceOptions::default()
        };
        (format!("IMAGES/I{at:04}"), classic_slice(&opts))
    });
    indexed(files.collect()).series[0].info.clone()
}

#[test]
fn a_series_says_whether_the_files_gave_the_distance_between_its_slices() {
    // Placed in space: the positions say.
    assert!(series_of(&SliceOptions::default()).slice_step_given);
    // Not placed, but each slice says how thick it is.
    let thick = unplaced(3, Some(4.0));
    assert!(thick.slice_step_given);
    assert_eq!(thick.slice_thickness_mm(), 4.0);
    // Not placed and no thickness either: the 1 mm is a stand-in, and the series says so.
    let nothing = unplaced(3, None);
    assert!(!nothing.slice_step_given);
    assert_eq!(nothing.slice_thickness_mm(), 1.0);
    // A thickness of nothing is none.
    assert!(!unplaced(3, Some(0.0)).slice_step_given);
    // One slice has no distance to another, placed or not.
    assert!(!unplaced(1, None).slice_step_given);
}
