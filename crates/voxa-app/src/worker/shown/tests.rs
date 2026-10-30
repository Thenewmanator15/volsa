//! A slice's preset windows are worked out when it is shown, not when it is decoded for a
//! volume.

use crate::action::Action;
use crate::effect::Effect;
use crate::testing::{THICK_SCAN, id};
use crate::worker::tests::{Bench, folder_of};
use crate::worker::volume_tests::{build, counted, stored, until_the_volume_ends};
use voxa_volume::PRESETS;

#[test]
fn volume_slices_report_no_windows_while_building() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let reports = until_the_volume_ends(&bench);
    let frames = reports
        .iter()
        .filter(|report| matches!(report, Action::FrameReady { .. }))
        .count();
    assert_eq!(frames, 0, "{reports:?}");
}

#[test]
fn a_held_series_reports_windows_for_the_slice_shown() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    until_the_volume_ends(&bench);
    let shown = Effect::RequestFrames {
        series: id(THICK_SCAN),
        around: 7,
    };
    bench.workers.run(shown, Some(&index));
    let volume = stored(&bench, THICK_SCAN).expect("kept");
    let values = volume.slice_values(7).expect("there is such a slice");
    let windows = PRESETS.map(|preset| preset.window(&values));
    let wanted = Action::FrameReady {
        series: id(THICK_SCAN),
        slice: 7,
        windows,
    };
    assert_eq!(bench.next(), wanted);
    assert!(bench.is_quiet(), "and nothing else: {:?}", bench.drain());
}
