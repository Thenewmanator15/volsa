//! Made-up labels for the phantom's ball, as another program might have written them: a
//! label volume on a grid of its own, and the table of names that goes with it.

use volsa_labels::testkit::{NiftiOptions, gzipped, nifti};

/// Nothing: outside the ball.
pub const NOTHING: u8 = 0;
/// The inner half of the ball's radius.
const CORE: u8 = 1;
/// The rest of the ball, towards the patient's left and towards their right.
const SHELL_LEFT: u8 = 2;
const SHELL_RIGHT: u8 = 3;
/// The small bright bead near the front.
pub const BEAD: u8 = 4;

/// The side of the label volume, in voxels, and of one of its voxels, in millimetres.
const SIDE: u16 = 64;
const VOXEL: f32 = 1.5;
/// Where the first label voxel lies, as a NIfTI file says it: millimetres towards the
/// patient's right, their front and their head.
const FIRST: [f32; 3] = [-47.25, -47.25, -0.45];

/// The table of names, in the shape a labelling pipeline writes: a name with its group, its
/// colour and a number for each side it has, one of them marked approximate, and a check of
/// the labels that fails. All of it is made up.
pub const TABLE: &str = r##"{
  "title": "Made-up phantom",
  "model": "made-up",
  "regions": {
    "shell": {"group": "Outside", "colour": "#4f8fd9", "ids": {"left": 2, "right": 3}},
    "core": {"group": "Inside", "colour": "#d9a441", "ids": {"midline": 1}},
    "bead": {
      "group": "Outside",
      "colour": "#e0523f",
      "ids": {"midline": 4},
      "approximate": true,
      "how": "Drawn round by rule, not found by a model."
    }
  },
  "qc": {"scores": {"core": 0.9, "shell": 0.52}, "fail_below": 0.65, "passed": false}
}
"##;

/// The part of the phantom at a point, each coordinate running from -1 to 1: across towards
/// the patient's left, down towards the back, height towards the head.
pub fn part(across: f64, down: f64, height: f64) -> u8 {
    let from_centre = (across * across + down * down + height * height).sqrt();
    let bead = ((across - 0.35).powi(2) + (down + 0.55).powi(2) + height * height).sqrt();
    if bead < 0.08 {
        BEAD
    } else if from_centre < 0.4 {
        CORE
    } else if from_centre >= 0.8 {
        NOTHING
    } else if across > 0.0 {
        SHELL_LEFT
    } else {
        SHELL_RIGHT
    }
}

/// The label volume for a series of the phantom whose pixels are `pixel` millimetres and
/// whose slices are `apart` millimetres apart: a gzipped NIfTI file. Its voxels are 1.5 mm
/// and run towards the head, then the front, then the patient's right, so neither their
/// size nor their order is the scan's.
pub fn volume(pixel: f64, apart: f64) -> Vec<u8> {
    let side = usize::from(SIDE);
    let mm = |first: f32, place: u16| f64::from(first) + f64::from(VOXEL) * f64::from(place);
    let mut ids = Vec::with_capacity(side * side * side);
    for slice in 0..SIDE {
        for row in 0..SIDE {
            for column in 0..SIDE {
                let (right, front, up) =
                    (mm(FIRST[0], slice), mm(FIRST[1], row), mm(FIRST[2], column));
                // In the scan a pixel's middle is half a pixel short of where its share of
                // the phantom's width is, and a slice's half a slice.
                let across = (pixel / 2.0 - right) / 48.0;
                let down = (pixel / 2.0 - front) / 48.0;
                let height = (up + apart / 2.0) / 48.0 - 1.0;
                ids.push(u32::from(part(across, down, height)));
            }
        }
    }
    let srow = [
        [0.0, 0.0, VOXEL, FIRST[0]],
        [0.0, VOXEL, 0.0, FIRST[1]],
        [VOXEL, 0.0, 0.0, FIRST[2]],
    ];
    gzipped(&nifti(&NiftiOptions {
        dims: [SIDE; 3],
        ids,
        kind: 2,
        srow: Some(srow),
        quatern: None,
        big_endian: false,
        slope: 1.0,
    }))
}
