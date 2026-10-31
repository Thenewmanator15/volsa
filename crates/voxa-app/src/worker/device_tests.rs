//! What the workers leave to the graphics device. They hand it nothing themselves.

use super::tests::{Bench, folder_of};
use super::volume_tests::{build, counted, stored, until_the_volume_ends};
use super::*;
use crate::testing::{THICK_SCAN, id};
use voxa_core::Grid;

#[test]
fn a_volume_the_card_can_hold_has_no_copy_made_for_it() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    until_the_volume_ends(&bench);
    assert!(lock(&bench.volumes).for_card(&id(THICK_SCAN)).is_none());
    assert!(
        stored(&bench, THICK_SCAN).is_some(),
        "the card holds the volume itself"
    );
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
    bench.workers.leave_to_the_device(true);
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
    let _ = queue;
    let bench = Bench::with_budget(1 << 20);
    bench
        .workers
        .set_card_room(voxa_render::CardRoom::of(&device, None));
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
