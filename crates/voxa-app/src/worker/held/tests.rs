//! A series is decoded straight into a volume held as its values are stored.

use voxa_core::{Grid, Stored};
use voxa_dicom::ImageFile;

use crate::action::Action;
use crate::testing::THICK_SCAN;
use crate::worker::tests::{Bench, folder_of};
use crate::worker::volume_tests::{build, counted, stored, until_the_volume_ends};

/// Build a series of 30 slices of 4 x 4 counted values, each slice's options changed by
/// `change`, and give the files and the reports.
/// The files of a series, each a name and its bytes.
type Files = Vec<(String, Vec<u8>)>;

fn built(
    change: impl Fn(u16, &mut voxa_dicom::testkit::SliceOptions),
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
fn reads_as_decoded(bench: &Bench, files: &[(String, Vec<u8>)]) {
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
