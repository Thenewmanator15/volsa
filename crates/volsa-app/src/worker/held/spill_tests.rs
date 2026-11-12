//! A series past the memory budget puts finished slabs away in Volsa's own file, and reads the
//! same values; one within it never does.

use super::tests::{Files, reads_as_decoded};
use crate::action::Action;
use crate::testing::THICK_SCAN;
use crate::worker::tests::{Bench, folder_of};
use crate::worker::volume_tests::{build, counted, stored, until_the_volume_ends};

/// Slices of 4 x 4 at 2 bytes, five a slab: 160 bytes a slab, six slabs for 30 slices.
const SLAB: usize = 5 * 32;

/// Build the 30 slices with slabs of five, a budget of `budget` and Volsa's folder `local`.
fn built_in(budget: usize, local: &std::path::Path) -> (Bench, Files, Vec<Action>) {
    let bench = Bench::keeping_in(budget, Some(local));
    bench.workers.set_slab_bytes(SLAB);
    let files = counted(THICK_SCAN, 30, |_, _| {});
    let folder = folder_of(&files);
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let reports = until_the_volume_ends(&bench);
    (bench, files, reports)
}

fn spill_files(local: &std::path::Path) -> usize {
    std::fs::read_dir(local.join("cache")).map_or(0, Iterator::count)
}

#[test]
fn a_series_within_the_budget_never_spills() {
    let local = tempfile::tempdir().expect("a temporary folder");
    let (bench, _, reports) = built_in(1 << 20, local.path());
    assert!(matches!(reports.last(), Some(Action::VolumeReady { .. })));
    let held = stored(&bench, THICK_SCAN).expect("kept");
    assert!((0..held.slab_count()).all(|slab| !held.is_away(slab)));
    assert_eq!(spill_files(local.path()), 0, "no file is made");
}

#[test]
fn a_series_past_the_budget_spills_and_reads_the_same_values() {
    let local = tempfile::tempdir().expect("a temporary folder");
    let (bench, files, reports) = built_in(2 * SLAB, local.path());
    assert!(
        matches!(reports.last(), Some(Action::VolumeReady { .. })),
        "{reports:?}"
    );
    let held = stored(&bench, THICK_SCAN).expect("kept");
    assert_eq!(held.slab_count(), 6);
    assert!(
        held.bytes_held() <= 3 * SLAB,
        "{} bytes held",
        held.bytes_held()
    );
    assert_eq!(spill_files(local.path()), 1);
    reads_as_decoded(&bench, &files);
    bench.until(|bench| bench.workers.over_budget() == 0);
    assert!(
        held.bytes_held() <= 3 * SLAB,
        "back within the budget after reading"
    );
    drop(held);
    bench.volumes.lock().expect("no worker panicked").clear();
    assert_eq!(
        spill_files(local.path()),
        0,
        "the file goes with its volume"
    );
}

#[test]
fn when_the_spill_file_cannot_be_made_the_volume_stays_and_a_notice_says_so() {
    let local = tempfile::tempdir().expect("a temporary folder");
    let not_a_folder = local.path().join("in the way");
    std::fs::write(&not_a_folder, b"a file").expect("written");
    let (bench, files, reports) = built_in(2 * SLAB, &not_a_folder);
    let said = "Volsa could not make room on disk, so this series uses more memory than the \
                amount set.";
    let told: Vec<&Action> = reports
        .iter()
        .filter(|report| matches!(report, Action::Post(notice) if notice.text == said))
        .collect();
    assert_eq!(told.len(), 1, "said once: {reports:?}");
    assert!(matches!(reports.last(), Some(Action::VolumeReady { .. })));
    let held = stored(&bench, THICK_SCAN).expect("kept");
    assert_eq!(held.bytes_held(), 6 * SLAB, "all of it in memory");
    reads_as_decoded(&bench, &files);
}
