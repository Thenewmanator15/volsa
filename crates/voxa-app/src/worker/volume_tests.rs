//! Building volumes on the workers: every slice decoded, stacked, and kept in the store.

use voxa_core::Volume;
use voxa_dicom::testkit::{SliceOptions, classic_slice};

use super::tests::{Bench, folder_of};
use super::*;
use crate::testing::{FINE_SCAN, THICK_SCAN, id, series_files, slice_options};

/// The files of a series of 4 x 4 slices whose values count up through the whole stack, with
/// `change` applied to each slice's options.
pub(super) fn counted(
    uid: &str,
    slices: u16,
    change: impl Fn(u16, &mut SliceOptions),
) -> Vec<(String, Vec<u8>)> {
    let file = |place: u16| {
        let mut options = SliceOptions {
            stored: (0..16).map(|value| place * 16 + value).collect(),
            ..slice_options(uid, 1, i32::from(place), 4)
        };
        change(place, &mut options);
        (format!("S1/I{place:04}.dcm"), classic_slice(&options))
    };
    (0..slices).map(file).collect()
}

pub(super) fn build(bench: &Bench, index: &Index, series: &str) {
    bench
        .workers
        .run(Effect::BuildVolume { series: id(series) }, Some(index));
}

/// The reports up to and including the first that says how a volume ended.
pub(super) fn until_the_volume_ends(bench: &Bench) -> Vec<Action> {
    let mut reports = Vec::new();
    loop {
        reports.push(bench.next());
        if let Some(Action::VolumeReady { .. } | Action::VolumeFailed { .. }) = reports.last() {
            return reports;
        }
    }
}

pub(super) fn stored(bench: &Bench, series: &str) -> Option<Arc<Volume>> {
    bench
        .volumes
        .lock()
        .expect("no worker panicked")
        .get(&id(series))
}

#[test]
fn a_volume_is_built_from_every_slice_and_kept_in_the_store() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let reports = until_the_volume_ends(&bench);

    let volume = stored(&bench, THICK_SCAN).expect("the volume is kept");
    assert_eq!(volume.dims(), [4, 4, 30]);
    assert_eq!(
        volume.get(1, 0, 29),
        Some(465.0),
        "the second value of the last slice"
    );
    let ready = Action::VolumeReady {
        series: id(THICK_SCAN),
        corners: volume.corners(),
    };
    assert_eq!(reports.last(), Some(&ready));

    let progress: Vec<usize> = reports
        .iter()
        .filter_map(|report| match report {
            Action::VolumeProgress {
                series,
                done,
                of: 30,
            } if *series == id(THICK_SCAN) => Some(*done),
            _ => None,
        })
        .collect();
    let other = |report: &&Action| {
        let about_the_volume = matches!(report, Action::VolumeProgress { .. });
        !about_the_volume && !matches!(report, Action::FrameReady { .. })
    };
    assert_eq!(
        reports.iter().filter(other).count(),
        1,
        "besides progress and slices, only that it is ready: {reports:?}"
    );
    assert!(
        progress.is_sorted(),
        "progress never goes down: {progress:?}"
    );
    assert!(
        progress.len() <= 3,
        "nor is it reported for every slice: {progress:?}"
    );
    assert!(
        progress.iter().all(|done| done % 8 == 0 && *done < 30),
        "{progress:?}"
    );
}

#[test]
fn slices_already_decoded_are_taken_from_the_cache() {
    let bench = Bench::with_budget(1 << 20);
    let files = counted(THICK_SCAN, 30, |_, _| {});
    let folder = folder_of(&files);
    let index = bench.index(folder.path()).expect("indexes");
    bench.frames(&index, THICK_SCAN, 15, 30);
    // With the files gone, only the cache can supply the slices.
    for (name, _) in &files {
        std::fs::remove_file(folder.path().join(name)).expect("removes");
    }
    build(&bench, &index, THICK_SCAN);
    let reports = until_the_volume_ends(&bench);
    assert!(
        matches!(reports.last(), Some(Action::VolumeReady { .. })),
        "{reports:?}"
    );
    assert_eq!(
        stored(&bench, THICK_SCAN).and_then(|volume| volume.get(0, 0, 2)),
        Some(32.0)
    );
}

#[test]
fn a_slice_that_will_not_decode_fails_the_volume_and_is_named() {
    let bench = Bench::with_budget(1 << 20);
    // The seventh slice has lost the end of its pixel data.
    let cut_short = |place: u16, options: &mut SliceOptions| {
        options.drop_pixels = if place == 6 { 3 } else { 0 };
    };
    let folder = folder_of(&counted(THICK_SCAN, 30, cut_short));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let reports = until_the_volume_ends(&bench);
    let Some(Action::VolumeFailed { series, reason }) = reports.last() else {
        panic!("{reports:?}");
    };
    assert_eq!(*series, id(THICK_SCAN));
    let said = "Slice 7 could not be decoded, so a 3D view cannot be built";
    assert!(
        reason.starts_with(said) && reason.len() > said.len() + 4,
        "{reason}"
    );
    assert!(stored(&bench, THICK_SCAN).is_none(), "nothing is kept");
    assert!(bench.is_quiet(), "and that is the only end");
}

#[test]
fn opening_another_export_leaves_nothing_of_a_volume_that_was_building() {
    let bench = Bench::with_budget(1 << 20);
    let first = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(first.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let second = folder_of(&series_files(FINE_SCAN, 2, 4, 8));
    bench
        .workers
        .run(Effect::StartIndex(second.path().to_owned()), None);
    loop {
        // The first volume may or may not have been finished and announced in time.
        if bench.next() == Action::IndexDone {
            break;
        }
    }
    assert!(
        stored(&bench, THICK_SCAN).is_none(),
        "the store was emptied for the new export"
    );
    assert!(bench.is_quiet(), "and the old build says no more");
    assert!(stored(&bench, THICK_SCAN).is_none());
}

#[test]
fn a_request_for_another_series_leaves_only_that_ones_volume() {
    let bench = Bench::with_budget(1 << 20);
    let files = [
        counted(THICK_SCAN, 30, |_, _| {}),
        series_files(FINE_SCAN, 2, 12, 8),
    ]
    .concat();
    let folder = folder_of(&files);
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    build(&bench, &index, FINE_SCAN);
    loop {
        match bench.next() {
            Action::VolumeReady { series, .. } if series == id(FINE_SCAN) => break,
            Action::VolumeFailed { reason, .. } => panic!("{reason}"),
            _ => {}
        }
    }
    assert!(bench.is_quiet());
    assert!(
        stored(&bench, THICK_SCAN).is_none(),
        "one volume is kept, the one last asked for"
    );
    assert_eq!(
        stored(&bench, FINE_SCAN).map(|volume| volume.dims()),
        Some([8, 8, 12])
    );
}

#[test]
fn a_volume_is_not_built_for_a_series_that_is_not_there() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 3, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, FINE_SCAN);
    bench.workers.run(
        Effect::BuildVolume {
            series: id(THICK_SCAN),
        },
        None,
    );
    assert!(bench.is_quiet());
}

#[test]
fn the_workers_carry_on_after_a_task_panics_and_still_build_a_volume() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 4, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    bench.workers.spawn(|| panic!("a reader gave up on a file"));
    build(&bench, &index, THICK_SCAN);
    assert!(matches!(
        until_the_volume_ends(&bench).last(),
        Some(Action::VolumeReady { .. })
    ));
}

#[test]
fn a_built_volume_is_also_made_ready_for_the_graphics_device() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    until_the_volume_ends(&bench);
    let take = || {
        let mut store = bench.volumes.lock().expect("no worker panicked");
        store.take_prepared(&id(THICK_SCAN))
    };
    let ready = take().expect("made ready on the worker");
    assert_eq!((ready.dims(), ready.halved()), ([4, 4, 30], 0));
    assert!(take().is_none(), "it is handed over once");
    assert!(
        stored(&bench, THICK_SCAN).is_some(),
        "the volume itself stays"
    );
}

#[test]
fn a_volume_is_made_ready_for_the_device_the_window_has() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    // A device that can hold no more than two voxels a side.
    bench.workers.set_volume_side(2);
    build(&bench, &index, THICK_SCAN);
    until_the_volume_ends(&bench);
    let mut store = bench.volumes.lock().expect("no worker panicked");
    let ready = store.take_prepared(&id(THICK_SCAN)).expect("made ready");
    assert_eq!((ready.dims(), ready.halved()), ([1, 1, 2], 4));
    assert!(
        store.take_prepared(&id(FINE_SCAN)).is_none(),
        "nor is it another series'"
    );
}

#[test]
fn with_the_blocks_left_to_the_device_a_volume_is_made_ready_without_them() {
    let made_ready = |leave: bool| {
        let bench = Bench::with_budget(1 << 20);
        let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
        let index = bench.index(folder.path()).expect("indexes");
        bench.workers.leave_to_the_device(leave, false);
        build(&bench, &index, THICK_SCAN);
        until_the_volume_ends(&bench);
        let ready = lock(&bench.volumes).take_prepared(&id(THICK_SCAN));
        ready.expect("made ready").has_blocks()
    };
    assert!(made_ready(false), "worked out on the worker");
    assert!(!made_ready(true), "left for the device to work out");
}

#[test]
fn a_curve_left_to_the_device_is_worked_out_on_a_worker_only_when_asked_outright() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    until_the_volume_ends(&bench);
    bench.drain();
    let series = id(THICK_SCAN);
    bench.workers.leave_to_the_device(false, true);
    let ask = Effect::BuildSmooth {
        series: series.clone(),
    };
    bench.workers.run(ask, Some(&index));
    assert!(
        bench.is_quiet(),
        "the device makes it from the volume it holds"
    );
    let taken = || lock(&bench.volumes).take_smoothed(&series);
    assert!(taken().is_none());
    // The device had no room after all: a worker is asked outright.
    bench.workers.work_out_curve(series.clone());
    let ready = Action::SmoothReady {
        series: series.clone(),
    };
    assert_eq!(bench.next(), ready);
    assert_eq!(taken().expect("kept").dims(), [4, 4, 30]);
}

#[test]
fn the_curve_through_the_volume_kept_is_worked_out_on_a_worker_and_kept_with_it() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    until_the_volume_ends(&bench);
    let series = id(THICK_SCAN);
    let ask = Effect::BuildSmooth {
        series: series.clone(),
    };
    bench.workers.run(ask.clone(), Some(&index));
    let ready = Action::SmoothReady {
        series: series.clone(),
    };
    assert_eq!(bench.drain().last(), Some(&ready));
    let taken = || lock(&bench.volumes).take_smoothed(&series);
    assert_eq!(taken().expect("kept").dims(), [4, 4, 30]);
    // Given up, a curve that arrives late is not kept.
    bench.workers.run(ask, Some(&index));
    assert_eq!(bench.next(), ready);
    bench.workers.run(Effect::DropSmooth, Some(&index));
    assert!(taken().is_none());
}

#[test]
fn no_curve_is_worked_out_for_a_series_whose_volume_is_not_the_one_kept() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 4, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    let ask = |series: &str| Effect::BuildSmooth { series: id(series) };
    bench.workers.run(ask(THICK_SCAN), Some(&index));
    assert!(bench.is_quiet(), "there is no volume yet");
    build(&bench, &index, THICK_SCAN);
    until_the_volume_ends(&bench);
    bench.drain();
    bench.workers.run(ask(FINE_SCAN), Some(&index));
    assert!(bench.is_quiet());
    assert!(lock(&bench.volumes).take_smoothed(&id(FINE_SCAN)).is_none());
}
