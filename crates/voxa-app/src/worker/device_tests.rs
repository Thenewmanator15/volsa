//! What the workers leave to the graphics device, and what they hand to it themselves.

use super::tests::{Bench, folder_of};
use super::volume_tests::{build, counted, stored, until_the_volume_ends};
use super::*;
use crate::testing::{THICK_SCAN, id};
use voxa_core::Grid;

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
fn given_the_graphics_device_a_worker_hands_the_volume_to_it_as_well_as_making_it_ready() {
    let made_ready = |bench: &Bench| {
        let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
        let index = bench.index(folder.path()).expect("indexes");
        build(bench, &index, THICK_SCAN);
        until_the_volume_ends(bench);
        let ready = lock(&bench.volumes).take_prepared(&id(THICK_SCAN));
        ready.expect("made ready")
    };
    let without = Bench::with_budget(1 << 20);
    assert!(
        !made_ready(&without).is_handed_over(),
        "no device to hand it to"
    );
    let Ok(mut scene) = voxa_render::HeadlessScene::new() else {
        eprintln!("skipped: no graphics adapter here, not even a software one");
        return;
    };
    let with = Bench::with_budget(1 << 20);
    let (device, queue) = scene.device();
    with.workers.give_device(device, queue);
    let ready = made_ready(&with);
    assert!(
        ready.is_handed_over(),
        "its values are on the device already"
    );
    scene.upload_prepared(&ready).expect("taken up");
    assert!(!ready.is_handed_over());
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
fn a_volume_is_still_made_and_reported_when_the_windows_device_is_lost() {
    let Some((device, queue)) = crate::testing::a_device() else {
        return;
    };
    // Watched as the window's is, and then lost.
    let watch = voxa_render::DeviceWatch::on(&device, || {});
    crate::testing::lose(&device);
    assert!(watch.is_lost());
    let bench = Bench::with_budget(1 << 20);
    bench.workers.give_device(device, queue);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let reports = until_the_volume_ends(&bench);
    let volume = stored(&bench, THICK_SCAN).expect("the volume is kept");
    let ready = Action::VolumeReady {
        series: id(THICK_SCAN),
        corners: volume.corners(),
    };
    assert_eq!(reports.last(), Some(&ready), "{reports:?}");
}
