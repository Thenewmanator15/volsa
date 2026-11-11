//! Series that hold several volumes: one image at every position for each echo, diffusion
//! weighting or time point.

use voxa_core::{SeriesId, SeriesKind, SeriesProblem, VolumeSet};

use super::tests::indexed;
use super::*;
use crate::testkit::{
    EnhancedFrame, EnhancedOptions, ScanTags, SliceOptions, classic_slice, enhanced_series,
};

/// The name a made-up image is filed under: an order that is neither that of the positions
/// nor that of the volumes.
fn filed(place: i32, volume: usize) -> String {
    let number = place * 10 + i32::try_from(volume).expect("a few") + 1;
    format!("F{:03}", (number * 37) % 101)
}

/// `places` positions 4 mm apart, each filled once for every entry of `apart`, which is what
/// the image of that volume says of itself.
fn grid(places: i32, apart: &[ScanTags]) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    for place in 0..places {
        for (volume, scan) in apart.iter().enumerate() {
            let options = SliceOptions {
                instance_number: place * 10 + i32::try_from(volume).expect("a few") + 1,
                position: Some([0.0, 0.0, f64::from(place) * 4.0]),
                scan: scan.clone(),
                ..SliceOptions::default()
            };
            files.push((filed(place, volume), classic_slice(&options)));
        }
    }
    files
}

fn echo(ms: f64) -> ScanTags {
    ScanTags {
        echo_ms: Some(ms),
        ..ScanTags::default()
    }
}

fn at_time(temporal: i32, echo_ms: f64) -> ScanTags {
    ScanTags {
        temporal: Some(temporal),
        ..echo(echo_ms)
    }
}

fn names(index: &Index) -> Vec<String> {
    let said = |series: &IndexedSeries| series.info.set.as_ref().map(|set| set.what.clone());
    index.series.iter().filter_map(said).collect()
}

#[test]
fn a_series_with_two_echoes_at_every_position_is_listed_as_two_volumes() {
    let index = indexed(grid(3, &[echo(21.0), echo(83.0)]));
    let whole = SliceOptions::default().series_uid;
    assert_eq!(index.series.len(), 2);
    assert_eq!(index.studies[0].series.len(), 2, "the study lists both");
    for (at, series) in index.series.iter().enumerate() {
        let info = &series.info;
        assert_eq!(info.id, SeriesId(format!("{whole}@{}", at + 1)));
        assert_eq!(info.slices, 3);
        assert_eq!(info.problems, Vec::new(), "each is a regular stack");
        assert!(info.voxel_to_patient.is_some());
        assert_eq!(info.spacing_mm[2], 4.0);
        assert_eq!(info.kind(), SeriesKind::Volume);
        let set = info.set.as_ref().expect("one of a set");
        assert_eq!(
            (&set.of, set.at, set.count),
            (&SeriesId(whole.clone()), at, 2)
        );
    }
    assert_eq!(names(&index), ["echo 21 ms", "echo 83 ms"]);
    let echoes: Vec<_> = index.series.iter().map(|s| s.info.scan.echo_ms).collect();
    assert_eq!(
        echoes,
        [Some(21.0), Some(83.0)],
        "each says its own echo time"
    );
    let first: Vec<&str> = index.series[0]
        .slices
        .iter()
        .map(|s| s.entry.as_str())
        .collect();
    assert_eq!(
        first,
        [filed(0, 0), filed(1, 0), filed(2, 0)],
        "in order of position"
    );
}

#[test]
fn volumes_nothing_tells_apart_are_numbered_in_the_order_of_their_images() {
    let index = indexed(grid(3, &[ScanTags::default(), ScanTags::default()]));
    assert_eq!(names(&index), ["", ""]);
    let second: Vec<&str> = index.series[1]
        .slices
        .iter()
        .map(|s| s.entry.as_str())
        .collect();
    assert_eq!(second, [filed(0, 1), filed(1, 1), filed(2, 1)]);
}

#[test]
fn diffusion_weightings_are_named_by_their_b_value() {
    let weighted = |b_value| ScanTags {
        b_value: Some(b_value),
        ..ScanTags::default()
    };
    let index = indexed(grid(3, &[weighted(850.0), weighted(0.0)]));
    assert_eq!(
        names(&index),
        ["b 0", "b 850"],
        "in order of weighting, not of filing"
    );
}

#[test]
fn time_points_tell_volumes_apart_before_echo_times_do() {
    let index = indexed(grid(3, &[at_time(1, 83.0), at_time(2, 21.0)]));
    assert_eq!(names(&index), ["time 1", "time 2"]);
    assert_eq!(index.series[0].info.scan.echo_ms, Some(83.0));
}

#[test]
fn a_file_of_many_frames_is_split_by_what_its_frames_say() {
    let frames = (0..3)
        .flat_map(|place| {
            [850.0, 0.0].map(|b_value| EnhancedFrame {
                b_value: Some(b_value),
                ..EnhancedFrame::at([0.0, 0.0, f64::from(place) * 4.0])
            })
        })
        .collect();
    let options = EnhancedOptions {
        common: SliceOptions::default(),
        frames,
    };
    let index = indexed(vec![("whole.dcm".to_owned(), enhanced_series(&options))]);
    assert_eq!(names(&index), ["b 0", "b 850"]);
    let frames_of = |series: &IndexedSeries| -> Vec<Option<u32>> {
        series.slices.iter().map(|slice| slice.frame).collect()
    };
    assert_eq!(frames_of(&index.series[0]), [Some(1), Some(3), Some(5)]);
    assert_eq!(frames_of(&index.series[1]), [Some(0), Some(2), Some(4)]);
}

#[test]
fn a_series_with_an_image_missing_from_one_volume_stays_one_plain_stack() {
    let mut files = grid(3, &[echo(21.0), echo(83.0)]);
    files.retain(|(name, _)| *name != filed(1, 1));
    let index = indexed(files);
    assert_eq!(index.series.len(), 1);
    let info = &index.series[0].info;
    assert_eq!(info.problems, vec![SeriesProblem::RepeatedPositions]);
    assert_eq!((info.slices, &info.set), (5, &None));
}

#[test]
fn a_position_filled_more_often_than_the_others_stays_one_plain_stack() {
    let mut files = grid(3, &[echo(21.0), echo(83.0)]);
    let extra = SliceOptions {
        instance_number: 99,
        position: Some([0.0, 0.0, 4.0]),
        ..SliceOptions::default()
    };
    files.push(("extra".to_owned(), classic_slice(&extra)));
    let index = indexed(files);
    assert_eq!(index.series.len(), 1);
    assert_eq!(index.series[0].info.set, None);
}

#[test]
fn one_position_filled_four_times_is_four_single_images() {
    let times: Vec<ScanTags> = (1..=4).map(|time| at_time(time, 21.0)).collect();
    let index = indexed(grid(1, &times));
    assert_eq!(names(&index), ["time 1", "time 2", "time 3", "time 4"]);
    for series in &index.series {
        assert_eq!(
            (series.info.slices, series.info.kind()),
            (1, SeriesKind::Single)
        );
    }
}

#[test]
fn more_than_nine_volumes_stay_in_their_order() {
    let times: Vec<ScanTags> = (1..=11).map(|time| at_time(time, 21.0)).collect();
    let index = indexed(grid(2, &times));
    let places: Vec<usize> = index
        .series
        .iter()
        .filter_map(|s| s.info.set.as_ref())
        .map(|set| set.at)
        .collect();
    assert_eq!(places, (0..11).collect::<Vec<_>>());
    let listed: Vec<Option<&VolumeSet>> = index.studies[0]
        .series
        .iter()
        .map(|info| info.set.as_ref())
        .collect();
    assert_eq!(
        listed
            .iter()
            .flatten()
            .map(|set| set.at)
            .collect::<Vec<_>>(),
        (0..11).collect::<Vec<_>>()
    );
}

#[test]
fn of_two_volumes_alike_the_first_is_the_one_opened_by_itself() {
    let index = indexed(grid(10, &[echo(21.0), echo(83.0)]));
    let best = best_series(&index).expect("both qualify");
    assert_eq!(best.info.set.as_ref().map(|set| set.at), Some(0));
}
