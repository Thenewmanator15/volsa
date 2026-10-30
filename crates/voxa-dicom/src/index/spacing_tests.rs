//! What the index says about a series' pixel spacing and rescale type.

use voxa_core::SeriesInfo;

use super::tests::{indexed, stack};
use crate::testkit::SliceOptions;

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

#[test]
fn the_rescale_type_is_read_and_trimmed() {
    let named = series_of(&SliceOptions {
        rescale_type: Some("HU ".to_owned()),
        ..SliceOptions::default()
    });
    assert_eq!(named.rescale_type.as_deref(), Some("HU"));
    assert_eq!(series_of(&SliceOptions::default()).rescale_type, None);
}
