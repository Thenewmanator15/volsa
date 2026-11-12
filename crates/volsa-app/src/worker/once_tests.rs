//! A series is decoded once and held once: as its volume while that is the one kept, and as
//! frames in the cache otherwise.

use volsa_dicom::testkit::SliceOptions;

use super::tests::{Bench, folder_of};
use super::volume_tests::{build, counted, stored, until_the_volume_ends};
use super::*;
use crate::testing::{FINE_SCAN, THICK_SCAN, id, series_files};
use volsa_core::Grid;
use volsa_volume::PRESETS;

/// How many times each slice of a series was announced as decoded.
fn announced(reports: &[Action], series: &str, slices: usize) -> Vec<usize> {
    let mut times = vec![0; slices];
    for report in reports {
        match report {
            Action::FrameReady {
                series: of, slice, ..
            } if *of == id(series) => times[*slice] += 1,
            _ => {}
        }
    }
    times
}

fn held_in_the_cache(bench: &Bench, series: &str, slices: usize) -> usize {
    let held = |slice: &usize| bench.cached(series, *slice).is_some();
    (0..slices).filter(held).count()
}

#[test]
fn building_a_volume_keeps_no_second_copy() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    until_the_volume_ends(&bench);
    let held = held_in_the_cache(&bench, THICK_SCAN, 30);
    assert_eq!(
        held, 0,
        "the volume holds the slices; the frame cache has no copy"
    );
}

#[test]
fn once_a_volume_is_built_its_slices_are_not_decoded_again() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    until_the_volume_ends(&bench);
    let frames = Effect::RequestFrames {
        series: id(THICK_SCAN),
        around: 3,
    };
    bench.workers.run(frames, Some(&index));
    let reports = bench.drain();
    assert_eq!(
        announced(&reports, THICK_SCAN, 30)[3],
        1,
        "the slice shown is said to be ready, with its windows: {reports:?}"
    );
    assert_eq!(reports.len(), 1, "and nothing else: {reports:?}");
    assert_eq!(
        held_in_the_cache(&bench, THICK_SCAN, 30),
        0,
        "they are read from the volume"
    );
}

#[test]
fn while_a_volume_is_wanted_the_viewport_decodes_only_the_slice_it_shows() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    // The volume first and then the frames: the order the session hands them over in.
    build(&bench, &index, THICK_SCAN);
    let frames = Effect::RequestFrames {
        series: id(THICK_SCAN),
        around: 15,
    };
    bench.workers.run(frames, Some(&index));
    let mut reports = until_the_volume_ends(&bench);
    reports.extend(bench.drain());
    // Only the slice shown is said to be ready: the viewport decoded it, and the volume's
    // own slices are said only when shown.
    let mut once = vec![0; 30];
    once[15] = 1;
    assert_eq!(announced(&reports, THICK_SCAN, 30), once);
    assert!(held_in_the_cache(&bench, THICK_SCAN, 30) <= 1);
}

#[test]
fn a_volume_that_gives_way_to_another_leaves_its_slices_in_the_cache() {
    let bench = Bench::with_budget(1 << 20);
    let files = [
        counted(THICK_SCAN, 30, |_, _| {}),
        series_files(FINE_SCAN, 2, 12, 8),
    ];
    let folder = folder_of(&files.concat());
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    until_the_volume_ends(&bench);
    build(&bench, &index, FINE_SCAN);
    until_the_volume_ends(&bench);
    assert!(stored(&bench, THICK_SCAN).is_none());
    // The new volume is said to be ready first; then the old one's slices are put by.
    bench.until(|bench| held_in_the_cache(bench, THICK_SCAN, 30) == 30);
    let last = bench.cached(THICK_SCAN, 29).expect("held");
    assert_eq!((last.rows, last.cols, last.values[1]), (4, 4, 465.0));
    // Going back builds from the cache, with nothing decoded again.
    build(&bench, &index, THICK_SCAN);
    let again = until_the_volume_ends(&bench);
    assert_eq!(announced(&again, THICK_SCAN, 30), vec![0; 30]);
    assert_eq!(
        stored(&bench, THICK_SCAN).map(|volume| volume.dims()),
        Some([4, 4, 30])
    );
}

#[test]
fn slices_decoded_for_a_volume_that_failed_are_kept_for_the_viewport() {
    let bench = Bench::with_budget(1 << 20);
    let cut_short = |place: u16, options: &mut SliceOptions| {
        options.drop_pixels = if place == 6 { 3 } else { 0 };
    };
    let folder = folder_of(&counted(THICK_SCAN, 30, cut_short));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let reports = until_the_volume_ends(&bench);
    assert!(
        matches!(reports.last(), Some(Action::VolumeFailed { .. })),
        "{reports:?}"
    );
    let good: Vec<usize> = (0..30).filter(|slice| *slice != 6).collect();
    let times = announced(&reports, THICK_SCAN, 30);
    assert_eq!(times, vec![0; 30], "none is said while the volume is built");
    let held: Vec<usize> = (0..30)
        .filter(|slice| bench.cached(THICK_SCAN, *slice).is_some())
        .collect();
    assert_eq!(held, good, "what was decoded is there to show");
    // Asking for the series' frames now decodes only the slice that is missing, and says
    // the slice shown is ready, with the windows of what the cache holds.
    let after = bench.frames(&index, THICK_SCAN, 0, 2);
    let shown = bench.cached(THICK_SCAN, 0).expect("held");
    let windows = PRESETS.map(|preset| preset.window(&shown.values));
    let ready = Action::FrameReady {
        series: id(THICK_SCAN),
        slice: 0,
        windows,
    };
    assert!(after.contains(&ready), "{after:?}");
    assert!(
        after
            .iter()
            .any(|report| matches!(report, Action::FrameFailed { slice: 6, .. })),
        "{after:?}"
    );
    assert!(bench.is_quiet());
}

#[test]
fn slices_decoded_for_a_volume_that_was_overtaken_are_kept_for_the_viewport() {
    let bench = Bench::with_budget(1 << 20);
    let files = [
        counted(THICK_SCAN, 30, |_, _| {}),
        series_files(FINE_SCAN, 2, 12, 8),
    ];
    let folder = folder_of(&files.concat());
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    build(&bench, &index, FINE_SCAN);
    let mut reports = Vec::new();
    loop {
        reports.push(bench.next());
        let ended = matches!(reports.last(), Some(Action::VolumeReady { series, .. }) if *series == id(FINE_SCAN));
        if ended {
            break;
        }
    }
    reports.extend(bench.drain());
    // However far the first build got, none of what it decoded was thrown away.
    let times = announced(&reports, THICK_SCAN, 30);
    let lost: Vec<usize> = (0..30)
        .filter(|slice| times[*slice] > 0 && bench.cached(THICK_SCAN, *slice).is_none())
        .collect();
    assert_eq!(lost, Vec::<usize>::new(), "announced {times:?}");
}

#[test]
fn what_is_said_when_the_reader_stops_reads_as_one_sentence() {
    let said =
        "This series could not be read for a 3D view: the reader stopped on one of its files";
    assert_eq!(volume::READER_STOPPED, said);
}
