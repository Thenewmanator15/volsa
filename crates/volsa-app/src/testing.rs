//! Small synthetic exports shared by this crate's tests.

use volsa_core::SeriesId;
use volsa_dicom::testkit::{MemorySource, SliceOptions, classic_slice};
use volsa_dicom::{Index, index};

pub mod documents;
pub mod patient;
mod volumes;

pub use volumes::{cube, held_of};

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
    use volsa_core::Point;

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
pub fn halves_of_the_head() -> volsa_labels::LabelSet {
    use volsa_labels::{LabelSet, Part, Side, Trust};
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
        placement: volsa_core::Affine::from_columns(
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
    std::sync::Arc<volsa_labels::LabelSet>,
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

/// The whole series of the made-up scan of two echoes: three positions, each filled twice.
pub const TWO_ECHOES: &str = "1.2.3.6";
/// Its first volume, the echo at 21 ms.
pub const FIRST_ECHO: &str = "1.2.3.6@1";
/// Its second volume, the echo at 83 ms.
pub const SECOND_ECHO: &str = "1.2.3.6@2";

/// The files of the scan of two echoes: 4 x 4 slices at three positions 5 mm apart, an image
/// of each echo at each, series number 6.
pub fn two_echoes_files() -> Vec<(String, Vec<u8>)> {
    let echoes = [(1, 21.0), (2, 83.0)];
    let image = |place: i32, (which, echo_ms): (i32, f64)| {
        let mut options = slice_options(TWO_ECHOES, 6, place, 4);
        options.instance_number = place * 2 + which;
        options.scan.echo_ms = Some(echo_ms);
        let name = format!("S6/I{place}-{which}.dcm");
        (name, classic_slice(&options))
    };
    (0..3)
        .flat_map(|place| echoes.map(|echo| image(place, echo)))
        .collect()
}

/// A made-up series of pictures in colour, series number 7.
pub const COLOUR: &str = "1.2.3.7";

/// The files of the series in colour: two 2 x 2 pictures.
pub fn colour_files() -> Vec<(String, Vec<u8>)> {
    let pixels = vec![[200, 10, 20], [10, 190, 30], [20, 30, 180], [240, 240, 240]];
    (0..2)
        .map(|place| {
            let mut options = slice_options(COLOUR, 7, place, 2);
            options.colour = Some(volsa_dicom::testkit::Colour::of(pixels.clone()));
            (format!("S7/I{place:04}.dcm"), classic_slice(&options))
        })
        .collect()
}

/// Labels of a made-up head, four voxels each way, the first half 17 and the rest 53, and the
/// table that names them and says who made them, and when.
pub fn made_up_labels() -> (volsa_labels::LabelGrid, volsa_labels::Table) {
    use volsa_labels::{Entry, LabelGrid, Made, Side, Table, Trust};
    let entry = |side, id| Entry {
        name: "hippocampus".to_owned(),
        group: "Deep brain".to_owned(),
        side,
        ids: vec![id],
        colour: [220, 216, 20],
        trust: Trust::Model,
    };
    let grid = LabelGrid {
        dims: [4, 4, 4],
        ids: (0..64).map(|at| if at < 32 { 17 } else { 53 }).collect(),
        placement: volsa_core::Affine::from_columns(
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [6.25, 6.75, 8.5],
        ),
    };
    let table = Table {
        title: "A made-up head".to_owned(),
        entries: vec![entry(Side::Left, 17), entry(Side::Right, 53)],
        verdict: None,
        made: Some(Made {
            by: "Volsa 0.4.0".to_owned(),
            model: "SynthSeg 2.0, robust model".to_owned(),
            model_sum: "0123456789abcdef".repeat(4),
            device: "the processor".to_owned(),
            when: "2099-01-02".to_owned(),
            notes: vec!["A made-up note.".to_owned()],
        }),
    };
    (grid, table)
}

/// The bytes of the label file and the text of the table of [`made_up_labels`], as a run
/// keeps them.
pub fn written_labels() -> (Vec<u8>, String) {
    let (grid, table) = made_up_labels();
    let labels = volsa_labels::write_labels(&grid).expect("the labels are written");
    (
        labels,
        volsa_labels::write_table(&table).expect("the table is written"),
    )
}

/// A graphics device with no window and its queue, as the window's are, or `None` with a
/// printed notice on a computer with no graphics adapter, not even a software one. An adapter
/// that will not start is not a reason to skip a test, and stops it.
pub fn a_device() -> Option<(egui_wgpu::wgpu::Device, egui_wgpu::wgpu::Queue)> {
    match volsa_render::HeadlessScene::new() {
        Ok(scene) => Some(scene.device()),
        Err(volsa_render::RenderError::NoAdapter) => {
            eprintln!("skipped: no graphics adapter here, not even a software one");
            None
        }
        Err(other) => panic!("{other}"),
    }
}

/// Lose a graphics device, as a test may: destroy it, and wait for what it was doing to end,
/// which is when a destroyed device says so. A real reset is never provoked.
pub fn lose(device: &egui_wgpu::wgpu::Device) {
    device.destroy();
    let _ = device.poll(egui_wgpu::wgpu::PollType::wait_indefinitely());
}

/// A made-up picture of 16 x 16 pixels, series number 9, whose file says nothing of where it
/// lies in the patient.
pub const PICTURE: &str = "1.2.3.9";

/// The file of [`PICTURE`], giving this Pixel Spacing (between rows, then between columns) and
/// this detector's spacing, each if any.
pub fn picture_file(
    pixel_spacing: Option<[f64; 2]>,
    imager_pixel_spacing: Option<[f64; 2]>,
) -> (String, Vec<u8>) {
    let options = SliceOptions {
        position: None,
        orientation: None,
        pixel_spacing,
        imager_pixel_spacing,
        ..slice_options(PICTURE, 9, 0, 16)
    };
    ("S9/I0000.dcm".to_owned(), classic_slice(&options))
}

/// A state showing [`PICTURE`] in the first viewport, its file as [`picture_file`] makes it,
/// with its one slice decoded and shown.
pub fn showing_a_picture(
    pixel_spacing: Option<[f64; 2]>,
    imager_pixel_spacing: Option<[f64; 2]>,
) -> crate::state::AppState {
    use crate::action::Action;
    use crate::reducer::reduce;
    let file = picture_file(pixel_spacing, imager_pixel_spacing);
    let mut state = crate::state::AppState::default();
    reduce(&mut state, Action::IndexProgress(indexed(vec![file])));
    let (viewport, series) = (0, id(PICTURE));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    shown(&mut state);
    state
}

/// Say that the slice the active viewport shows has been decoded, as the worker says once it
/// has: it is then shown.
pub fn shown(state: &mut crate::state::AppState) {
    use crate::action::Action;
    use crate::reducer::reduce;
    let Some(viewport) = state.viewports.get(state.active) else {
        return;
    };
    let Some(series) = viewport.series.clone() else {
        return;
    };
    let windows = [volsa_core::WindowLevel::new(50.0, 100.0); volsa_volume::PRESETS.len()];
    let slice = viewport.slice;
    reduce(
        state,
        Action::FrameReady {
            series,
            slice,
            windows,
        },
    );
}

/// A made-up single slice, series number 8, that says what kind of scan it is.
pub const KIND_SCAN: &str = "1.2.3.8";

/// A state showing a made-up single 4 x 4 slice of this modality in the first viewport, whose
/// file gives this Rescale Type, if any.
pub fn showing_a_scan_of(modality: &str, rescale_type: Option<&str>) -> crate::state::AppState {
    use crate::action::Action;
    use crate::reducer::reduce;
    let options = SliceOptions {
        modality: modality.to_owned(),
        rescale_type: rescale_type.map(str::to_owned),
        ..slice_options(KIND_SCAN, 8, 0, 4)
    };
    let files = vec![("S8/I0000.dcm".to_owned(), classic_slice(&options))];
    let mut state = crate::state::AppState::default();
    reduce(&mut state, Action::IndexProgress(indexed(files)));
    let (viewport, series) = (0, id(KIND_SCAN));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    state
}

/// The graphics device as the window has it, made on the one adapter every device with no window
/// shares; `None` with a printed notice on a computer with no graphics adapter, not even a
/// software one. An adapter that will not start is not a reason to skip a test, and stops it.
pub fn a_render_state() -> Option<egui_wgpu::RenderState> {
    match volsa_render::render_state_without_a_window() {
        Ok(render) => Some(render),
        Err(volsa_render::RenderError::NoAdapter) => {
            eprintln!("skipped: no graphics adapter here, not even a software one");
            None
        }
        Err(other) => panic!("{other}"),
    }
}
