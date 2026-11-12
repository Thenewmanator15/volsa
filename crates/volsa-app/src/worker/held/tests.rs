//! A series is decoded straight into a volume held as its values are stored.

use volsa_core::{Grid, Stored};
use volsa_dicom::ImageFile;

use crate::action::Action;
use crate::testing::THICK_SCAN;
use crate::worker::tests::{Bench, folder_of};
use crate::worker::volume_tests::{build, counted, stored, until_the_volume_ends};

/// Build a series of 30 slices of 4 x 4 counted values, each slice's options changed by
/// `change`, and give the files and the reports.
/// The files of a series, each a name and its bytes.
pub(super) type Files = Vec<(String, Vec<u8>)>;

fn built(
    change: impl Fn(u16, &mut volsa_dicom::testkit::SliceOptions),
) -> (Bench, Files, Vec<Action>) {
    let bench = Bench::with_budget(1 << 20);
    let files = counted(THICK_SCAN, 30, change);
    let folder = folder_of(&files);
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let reports = until_the_volume_ends(&bench);
    (bench, files, reports)
}

/// Every value of the held volume equals, to the bit, what decoding its file gives.
pub(super) fn reads_as_decoded(bench: &Bench, files: &[(String, Vec<u8>)]) {
    let held = stored(bench, THICK_SCAN).expect("kept");
    for (slice, (_, bytes)) in files.iter().enumerate() {
        let decoded = ImageFile::open(bytes)
            .and_then(|file| file.decode(None))
            .expect("decodes");
        let at = u32::try_from(slice).expect("small");
        let values = held.slice_values(at).expect("a slice");
        let bits = |values: &[f32]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&values), bits(&decoded.values), "slice {slice}");
    }
}

#[test]
fn a_ct_series_is_held_at_sixteen_bits_and_reads_its_hounsfield_numbers() {
    let (bench, files, reports) = built(|_, options| {
        options.modality = "CT".to_owned();
        options.intercept = -1024.0;
    });
    assert!(
        matches!(reports.last(), Some(Action::VolumeReady { .. })),
        "{reports:?}"
    );
    let held = stored(&bench, THICK_SCAN).expect("kept");
    assert_eq!(held.stored(), Stored::Unsigned16);
    assert_eq!(held.value(1, 0, 2), Some(33.0 - 1024.0));
    reads_as_decoded(&bench, &files);
}

#[test]
fn a_series_whose_files_differ_in_rescale_is_held_in_thirty_two_bits_with_the_same_values() {
    let (bench, files, reports) = built(|place, options| {
        options.slope = if place < 15 { 1.0 } else { 2.0 };
    });
    assert!(
        matches!(reports.last(), Some(Action::VolumeReady { .. })),
        "{reports:?}"
    );
    let held = stored(&bench, THICK_SCAN).expect("kept");
    assert_eq!(held.stored(), Stored::Float32);
    reads_as_decoded(&bench, &files);
}

#[test]
fn a_monochrome1_series_is_held_in_thirty_two_bits() {
    let (bench, files, _) = built(|_, options| options.monochrome1 = true);
    let held = stored(&bench, THICK_SCAN).expect("kept");
    assert_eq!(held.stored(), Stored::Float32);
    reads_as_decoded(&bench, &files);
}

/// Build a series of `slices` counted slices and give the files and the reports.
fn built_long(slices: u16) -> (Bench, Files, Vec<Action>) {
    let bench = Bench::with_budget(1 << 20);
    let files = counted(THICK_SCAN, slices, |_, _| {});
    let folder = folder_of(&files);
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let reports = until_the_volume_ends(&bench);
    (bench, files, reports)
}

#[test]
fn a_long_series_reports_a_preview_before_its_volume_and_the_volume_is_still_exact() {
    let (bench, files, reports) = built_long(300);
    let previewed = reports
        .iter()
        .position(|report| matches!(report, Action::PreviewReady { .. }))
        .expect("a preview");
    assert!(matches!(reports.last(), Some(Action::VolumeReady { .. })));
    assert!(previewed + 1 < reports.len());
    let preview = bench
        .volumes
        .lock()
        .expect("no worker panicked")
        .take_preview(&crate::testing::id(THICK_SCAN));
    assert!(preview.is_none(), "given up once the volume itself is kept");
    reads_as_decoded(&bench, &files);
}

#[test]
fn a_preview_is_kept_for_the_window_until_the_volume_is() {
    let bench = Bench::with_budget(1 << 20);
    let files = counted(THICK_SCAN, 300, |_, _| {});
    let folder = folder_of(&files);
    let index = bench.index(folder.path()).expect("indexes");
    build(&bench, &index, THICK_SCAN);
    let series = crate::testing::id(THICK_SCAN);
    let corners = loop {
        if let Action::PreviewReady { corners, .. } = bench.next() {
            break corners;
        }
    };
    let taken = bench
        .volumes
        .lock()
        .expect("no worker panicked")
        .take_preview(&series);
    // Taken before the volume is kept, unless the build was quicker than this test.
    if let Some(preview) = taken {
        assert_eq!(
            preview.dims(),
            [1, 1, 38],
            "4 x 4 slices in blocks of 4, every 8th of 300"
        );
    }
    until_the_volume_ends(&bench);
    let whole = stored(&bench, THICK_SCAN).expect("kept");
    assert_eq!(
        whole.corners(),
        corners,
        "the preview is said to lie where the series does"
    );
}

#[test]
fn a_series_of_256_slices_or_fewer_reports_no_preview() {
    let (_, _, reports) = built_long(200);
    assert!(
        !reports
            .iter()
            .any(|report| matches!(report, Action::PreviewReady { .. })),
        "{reports:?}"
    );
}

#[test]
fn a_slice_a_viewport_asks_for_is_decoded_while_every_building_thread_is_busy() {
    // LS-12: the slice on screen is never left waiting behind the slices of a volume.
    let bench = Bench::with_budget(1 << 20);
    let files = counted(THICK_SCAN, 30, |_, _| {});
    let folder = folder_of(&files);
    let index = bench.index(folder.path()).expect("indexes");
    let (release, wait) = std::sync::mpsc::channel::<()>();
    let wait = std::sync::Arc::new(std::sync::Mutex::new(wait));
    let threads = bench
        .workers
        .builds
        .as_ref()
        .map_or(1, rayon::ThreadPool::current_num_threads);
    for _ in 0..threads {
        let wait = std::sync::Arc::clone(&wait);
        bench.workers.spawn_build(move || {
            let _ = wait.lock().map(|wait| wait.recv());
        });
    }
    let reports = bench.frames(&index, THICK_SCAN, 12, 1);
    drop(release);
    assert!(
        matches!(reports.first(), Some(Action::FrameReady { slice: 12, .. })),
        "{reports:?}"
    );
}
