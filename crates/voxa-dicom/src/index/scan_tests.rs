//! What the index learns of how a series was acquired.

use super::tests::indexed;
use crate::testkit::{
    EnhancedFrame, EnhancedOptions, ScanTags, SliceOptions, classic_slice, enhanced_series,
};

/// Three axial slices of a made-up series whose files say how it was acquired.
fn told(scan: &ScanTags) -> Vec<(String, Vec<u8>)> {
    (0..3)
        .map(|place| {
            let options = SliceOptions {
                instance_number: place + 1,
                position: Some([0.0, 0.0, f64::from(place) * 4.0]),
                scan: scan.clone(),
                ..SliceOptions::default()
            };
            (format!("I{place}.dcm"), classic_slice(&options))
        })
        .collect()
}

#[test]
fn a_series_says_what_kind_of_scan_it_is_from_its_own_tags() {
    let scan = ScanTags {
        sequence: "IR\\SE".to_owned(),
        acquired: "2D".to_owned(),
        repetition_ms: Some(2345.0),
        echo_ms: Some(67.0),
        inversion_ms: Some(890.0),
        flip_deg: Some(123.0),
        field_t: Some(7.0),
        body_part: "KNEE".to_owned(),
        ..ScanTags::default()
    };
    let index = indexed(told(&scan));
    let facts = &index.series[0].info.scan;
    assert_eq!(
        facts.line().as_deref(),
        Some("Axial · 2D · inversion-recovery spin echo")
    );
    assert_eq!(
        (facts.repetition_ms, facts.echo_ms, facts.inversion_ms),
        (Some(2345.0), Some(67.0), Some(890.0))
    );
    assert_eq!((facts.flip_deg, facts.field_t), (Some(123.0), Some(7.0)));
    assert_eq!(facts.body_part, "KNEE");
}

#[test]
fn a_series_whose_files_say_nothing_of_the_scan_still_knows_its_plane() {
    let index = indexed(told(&ScanTags::default()));
    let facts = &index.series[0].info.scan;
    assert_eq!(facts.line().as_deref(), Some("Axial"));
    assert_eq!(facts.lines(), ["Plane: axial"]);
}

#[test]
fn a_file_of_many_frames_says_the_same_things_in_its_own_way() {
    let scan = ScanTags {
        sequence: "EP\\GR".to_owned(),
        acquired: "3D".to_owned(),
        repetition_ms: Some(612.0),
        echo_ms: Some(21.0),
        flip_deg: Some(17.0),
        ..ScanTags::default()
    };
    let options = EnhancedOptions {
        common: SliceOptions {
            scan,
            ..SliceOptions::default()
        },
        frames: (0..3)
            .map(|place| EnhancedFrame::at([0.0, 0.0, f64::from(place) * 4.0]))
            .collect(),
    };
    let index = indexed(vec![("whole.dcm".to_owned(), enhanced_series(&options))]);
    let facts = &index.series[0].info.scan;
    assert_eq!(
        facts.line().as_deref(),
        Some("Axial · 3D · echo-planar gradient echo")
    );
    assert_eq!(
        (facts.repetition_ms, facts.echo_ms, facts.flip_deg),
        (Some(612.0), Some(21.0), Some(17.0))
    );
}
