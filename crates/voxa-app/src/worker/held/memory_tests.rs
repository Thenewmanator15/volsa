//! Memory is checked before a volume is built, against what it will hold, not its size.

use voxa_models::FreeMemory;

use crate::action::{Action, Effect};
use crate::testing::{THICK_SCAN, id};
use crate::worker::tests::{Bench, folder_of};
use crate::worker::volume_tests::{build, counted, until_the_volume_ends};

/// What building was tried with: the workers, what they said, the index and its folder.
type Built = (Bench, Vec<Action>, voxa_dicom::Index, tempfile::TempDir);

/// Build the 30 slices of 4 x 4 (960 bytes at 16 bits) with this budget and free memory.
fn built_with(budget: usize, free: u64, slab_bytes: usize) -> Built {
    let bench = Bench::with_budget(budget);
    bench.workers.set_slab_bytes(slab_bytes);
    bench
        .workers
        .set_free_memory(move || FreeMemory::Promised(free));
    let folder = folder_of(&counted(THICK_SCAN, 30, |_, _| {}));
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let reports = until_the_volume_ends(&bench);
    (bench, reports, index, folder)
}

#[test]
fn too_little_memory_refuses_the_build_in_a_sentence() {
    let (_, reports, _, _) = built_with(1 << 20, 100 << 20, 32 << 20);
    let said = "Not enough memory to show this series in 3D: it needs 0.5 GB, and Windows can \
                promise Voxa 0.1 GB. Close other programs and open it again.";
    assert_eq!(
        reports.last(),
        Some(&Action::VolumeFailed {
            series: id(THICK_SCAN),
            reason: said.to_owned(),
        })
    );
}

#[test]
fn a_series_past_the_budget_is_not_refused_for_its_size() {
    // It needs the budget, a slab and the margin: 512 MiB and 224 bytes, of 512 MiB and 500.
    // Its whole 960 bytes would not have fitted.
    let (_, reports, _, _) = built_with(64, (512 << 20) + 500, 160);
    assert!(
        matches!(reports.last(), Some(Action::VolumeReady { .. })),
        "{reports:?}"
    );
}

#[test]
fn the_processors_curve_is_refused_past_the_budget() {
    // Its 32-bit copy is 1,920 bytes, past a budget of 1,024.
    let (bench, reports, index, _folder) = built_with(1024, 64 << 30, 32 << 20);
    assert!(matches!(reports.last(), Some(Action::VolumeReady { .. })));
    bench.drain();
    let ask = Effect::BuildSmooth {
        series: id(THICK_SCAN),
    };
    bench.workers.run(ask, Some(&index));
    assert_eq!(
        bench.next(),
        Action::Post(crate::planes::too_long_to_smooth())
    );
}
