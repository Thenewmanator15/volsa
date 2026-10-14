//! Small synthetic exports shared by this crate's tests.

use voxa_core::SeriesId;
use voxa_dicom::testkit::{MemorySource, SliceOptions, classic_slice};
use voxa_dicom::{Index, index};

/// Thirty 4 × 4 slices, series number 1.
pub const THICK_SCAN: &str = "1.2.3.1";
/// Sixty-four 8 × 8 slices, series number 2: the most detailed.
pub const FINE_SCAN: &str = "1.2.3.2";
/// Three 4 × 4 slices, series number 3.
pub const SCOUT: &str = "1.2.3.3";

/// Three 4 x 4 locating images, one in each plane, series number 4.
pub const LOCALISER: &str = "1.2.3.4";
/// A single 4 x 4 slice, series number 5.
pub const SINGLE: &str = "1.2.3.5";

pub fn id(text: &str) -> SeriesId {
    SeriesId(text.to_owned())
}

/// The options of slice `place` (counting from 0) of a series, 5 mm above the one before.
pub fn slice_options(uid: &str, number: i32, place: i32, side: u16) -> SliceOptions {
    SliceOptions {
        series_uid: uid.to_owned(),
        series_number: number,
        instance_number: place + 1,
        rows: side,
        cols: side,
        position: Some([0.0, 0.0, f64::from(place) * 5.0]),
        ..SliceOptions::default()
    }
}

/// The files of one series, named so that name order is slice order.
pub fn series_files(uid: &str, number: i32, count: i32, side: u16) -> Vec<(String, Vec<u8>)> {
    (0..count)
        .map(|place| {
            let bytes = classic_slice(&slice_options(uid, number, place, side));
            (format!("S{number}/I{place:04}.dcm"), bytes)
        })
        .collect()
}

pub fn three_series_files() -> Vec<(String, Vec<u8>)> {
    [
        series_files(THICK_SCAN, 1, 30, 4),
        series_files(FINE_SCAN, 2, 64, 8),
        series_files(SCOUT, 3, 3, 4),
    ]
    .concat()
}

pub fn indexed(files: Vec<(String, Vec<u8>)>) -> Index {
    index(&mut MemorySource::new(files), &mut |_| {}).expect("indexes")
}

/// The files of the three series, of a set of locating images, and of a series of one slice.
pub fn odd_series_files() -> Vec<(String, Vec<u8>)> {
    let planes = [
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0, 0.0, 0.0, -1.0],
        [0.0, 1.0, 0.0, 0.0, 0.0, -1.0],
    ];
    let locating = (0..3).zip(planes).map(|(place, plane)| {
        let options = SliceOptions {
            orientation: Some(plane),
            position: Some([0.0, 0.0, 0.0]),
            ..slice_options(LOCALISER, 4, place, 4)
        };
        (format!("S4/I{place:04}.dcm"), classic_slice(&options))
    });
    let files = [
        three_series_files(),
        locating.collect(),
        series_files(SINGLE, 5, 1, 4),
    ];
    files.concat()
}

/// The three series, a set of locating images, and a series of one slice.
pub fn with_odd_series() -> Index {
    indexed(odd_series_files())
}

pub fn three_series() -> Index {
    indexed(three_series_files())
}

/// A state with the three series listed, the 30-slice one shown, and its volume ready in the
/// scene: the volume's own corners are what the worker would have reported.
pub fn thick_scan_ready() -> crate::state::AppState {
    use voxa_core::Point;

    use crate::action::Action;
    use crate::reducer::reduce;
    let mut state = crate::state::AppState::default();
    reduce(&mut state, Action::IndexProgress(three_series()));
    let (viewport, series) = (0, id(THICK_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let placement = info.voxel_to_patient.expect("placed");
    let end = |i: usize, bit: usize, count: f64| if i & bit == 0 { -0.5 } else { count - 0.5 };
    let corners = std::array::from_fn(|i| {
        placement.apply(Point::new(end(i, 1, 4.0), end(i, 2, 4.0), end(i, 4, 30.0)))
    });
    let series = id(THICK_SCAN);
    reduce(&mut state, Action::VolumeReady { series, corners });
    state
}

/// Labels for the 30-slice series: two label voxels side by side that between them cover it,
/// the first, part 0, towards the patient's right and the second, part 1, towards their left.
pub fn halves_of_the_head() -> voxa_labels::LabelSet {
    use voxa_labels::{LabelSet, Part, Side, Trust};
    let part = |side| Part {
        name: "half".to_owned(),
        group: "Halves".to_owned(),
        side,
        colour: [200, 100, 50],
        trust: Trust::Model,
        voxels: 1,
        ml: 1.0,
    };
    LabelSet {
        dims: [2, 1, 1],
        numbers: vec![1, 2],
        placement: voxa_core::Affine::from_columns(
            [1.8, 0.0, 0.0],
            [0.0, 3.6, 0.0],
            [0.0, 0.0, 150.0],
            [0.45, 1.35, 72.5],
        ),
        parts: vec![part(Side::Right), part(Side::Left)],
        title: String::new(),
        verdict: None,
        made: None,
    }
}

/// The 30-slice series with its volume ready and [`halves_of_the_head`] read on it.
pub fn labelled_head() -> (
    crate::state::AppState,
    std::sync::Arc<voxa_labels::LabelSet>,
) {
    use crate::action::Action;
    use crate::reducer::reduce;
    let set = std::sync::Arc::new(halves_of_the_head());
    let mut state = thick_scan_ready();
    let open = Action::OpenLabels {
        labels: std::path::PathBuf::from("labels.nii"),
        table: None,
    };
    reduce(&mut state, open);
    let read = Action::LabelsRead {
        title: String::new(),
        parts: set.parts.clone(),
        verdict: None,
        made: None,
        corners: set.corners(),
        names_from: crate::labels::NamesFrom::Nothing,
        run: None,
    };
    reduce(&mut state, read);
    assert!(
        crate::labels::on_this_series(&state),
        "the labels meet the scan"
    );
    (state, set)
}
