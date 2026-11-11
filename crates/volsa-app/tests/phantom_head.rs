//! The made-up head that `write_phantom head` writes from a model folder's `check` folder is
//! the head that folder describes, in the place it describes, and Voxa reads it back exactly;
//! and with `whole` it lies in the same place in a grid of air as large as a whole-head scan.

use std::path::Path;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use voxa_app::{Action, NoticeKind, SceneStatus, Session};
use voxa_core::{Patient, Point, Volume};

#[path = "../examples/write_phantom/head.rs"]
mod head;

/// The made-up head's size in voxels along its columns, rows and slices.
const DIMS: [u32; 3] = [12, 16, 20];
/// Where its first voxel lies as a NIfTI file places it, in millimetres towards the patient's
/// right, front and head. Voxels are 1 mm apart along the axes.
const FIRST: [f64; 3] = [-9.25, -15.5, -11.75];
/// How long the workers may take over one thing before the test gives up.
const PATIENCE: Duration = Duration::from_secs(120);

/// The value of voxel `(col, row, slice)`: a ramp that 64 steps to the unit hold exactly.
fn ramp(col: u32, row: u32, slice: u32) -> f32 {
    (col + DIMS[0] * (row + DIMS[1] * slice)) as f32 / 8.0
}

/// Write a model folder's `check` folder for the made-up head: `case.json` and `volume.f32`.
fn write_check(folder: &Path) {
    let [x, y, z] = FIRST;
    let case = format!(
        r#"{{"volume": {{"dims": [{}, {}, {}], "affine":
            [[1.0, 0.0, 0.0, {x}], [0.0, 1.0, 0.0, {y}], [0.0, 0.0, 1.0, {z}], [0.0, 0.0, 0.0, 1.0]]}}}}"#,
        DIMS[0], DIMS[1], DIMS[2]
    );
    std::fs::write(folder.join("case.json"), case).expect("case.json is written");
    let mut values = Vec::new();
    for slice in 0..DIMS[2] {
        for row in 0..DIMS[1] {
            for col in 0..DIMS[0] {
                values.extend(ramp(col, row, slice).to_le_bytes());
            }
        }
    }
    std::fs::write(folder.join("volume.f32"), values).expect("volume.f32 is written");
}

/// Open a folder as Voxa does and give the volume of its one series.
fn volume_of(export: &Path) -> Volume {
    let (sender, reports) = channel();
    let mut session = Session::new(sender, reports, 1 << 30, None);
    session.dispatch(Action::OpenPath(export.to_owned()));
    let started = Instant::now();
    while session.state().scene.status != SceneStatus::Ready {
        let failed = session
            .state()
            .notices
            .iter()
            .any(|notice| notice.kind == NoticeKind::Error);
        assert!(!failed, "the folder could not be opened");
        assert!(
            started.elapsed() < PATIENCE,
            "the volume was not built in time"
        );
        session.wait(Duration::from_millis(50));
    }
    let series = session
        .state()
        .scene
        .series
        .clone()
        .expect("a series is in the scene");
    let kept = session.volume(&series).expect("the volume is kept");
    (*kept).clone()
}

/// Where voxel `(col, row, slice)` of the made-up head lies in the patient, in millimetres: the
/// NIfTI placement turned to DICOM's, whose x runs to the patient's left and y to the back.
fn expected(col: u32, row: u32, slice: u32) -> Point<Patient> {
    let [x, y, z] = FIRST;
    Point::new(
        -(x + f64::from(col)),
        -(y + f64::from(row)),
        z + f64::from(slice),
    )
}

fn close(found: Point<Patient>, wanted: Point<Patient>) -> bool {
    (found.x - wanted.x).abs() < 1e-6
        && (found.y - wanted.y).abs() < 1e-6
        && (found.z - wanted.z).abs() < 1e-6
}

#[test]
fn the_head_is_written_where_and_as_the_check_folder_describes_it() {
    let check = tempfile::tempdir().expect("a folder for the check folder");
    write_check(check.path());
    let out = tempfile::tempdir().expect("a folder to write into");
    head::write(check.path(), out.path(), false).expect("the head is written");
    let volume = volume_of(out.path());
    assert_eq!(volume.dims(), DIMS);
    for (col, row, slice) in [(0, 0, 0), (3, 5, 7), (11, 15, 19), (11, 0, 19)] {
        let at = volume.voxel_to_patient().apply(Point::new(
            f64::from(col),
            f64::from(row),
            f64::from(slice),
        ));
        assert!(
            close(at, expected(col, row, slice)),
            "voxel {col}, {row}, {slice} lies at {at:?}"
        );
    }
    for slice in 0..DIMS[2] {
        for row in 0..DIMS[1] {
            for col in 0..DIMS[0] {
                let got = volume.get(col, row, slice);
                assert_eq!(
                    got,
                    Some(ramp(col, row, slice)),
                    "voxel {col}, {row}, {slice}"
                );
            }
        }
    }
}

#[test]
fn the_head_in_a_whole_head_grid_lies_in_the_same_place_among_air() {
    let check = tempfile::tempdir().expect("a folder for the check folder");
    write_check(check.path());
    let out = tempfile::tempdir().expect("a folder to write into");
    head::write(check.path(), out.path(), true).expect("the head is written");
    let volume = volume_of(out.path());
    assert_eq!(volume.dims(), [176, 256, 256]);
    // The air before the head along each axis: half of what the grid has over the head.
    let before = [
        (176 - DIMS[0]) / 2,
        (256 - DIMS[1]) / 2,
        (256 - DIMS[2]) / 2,
    ];
    for (col, row, slice) in [(0, 0, 0), (3, 5, 7), (11, 15, 19)] {
        let grid = [col + before[0], row + before[1], slice + before[2]];
        let at = volume.voxel_to_patient().apply(Point::new(
            f64::from(grid[0]),
            f64::from(grid[1]),
            f64::from(grid[2]),
        ));
        assert!(
            close(at, expected(col, row, slice)),
            "the head moved: {at:?}"
        );
        let found = volume.get(grid[0], grid[1], grid[2]);
        assert_eq!(found, Some(ramp(col, row, slice)));
    }
    // Everywhere else is air: the ramp is nought only at its first voxel.
    let counted = volume
        .values()
        .iter()
        .filter(|value| **value != 0.0)
        .count();
    let in_the_head = (DIMS[0] * DIMS[1] * DIMS[2]) as usize;
    assert_eq!(
        counted,
        in_the_head - 1,
        "only the head holds anything but air"
    );
}
