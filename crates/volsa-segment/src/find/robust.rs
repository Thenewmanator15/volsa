// Ported from SynthSeg (https://github.com/BBillot/SynthSeg), Copyright 2020 Benjamin Billot,
// Apache License 2.0: `build_model` in SynthSeg/predict_synthseg.py, translated to Rust and changed
// for Volsa.
//! The robust networks: three, run one after another. The first finds a few broad classes in
//! the image. The second tidies that map. The third finds the structures, from the image and
//! the tidied map.

use volsa_models::{Role, RunError};

use super::{Halves, Image, Networks, Stage, map, run_once};

/// The chance of each of `count` structures at each voxel, the structures changing fastest,
/// as the third network gives it. Each network is dropped before the next is opened.
pub(super) fn chances(
    image: &Image,
    count: usize,
    networks: &mut dyn Networks,
    said: &mut dyn FnMut(Stage),
) -> Result<Vec<f32>, RunError> {
    let step = |step| Stage::Finding { step, of: 3 };
    let (sizes, a_map) = (image.size, image.map());
    said(step(1));
    let answer = run_once(networks, Role::Robust1, sizes, vec![image.copy()?])?;
    let classes = map(answer, &a_map)?;
    said(step(2));
    let answer = run_once(networks, Role::Robust2, sizes, vec![classes])?;
    let cleaned = map(answer, &a_map)?;
    said(step(3));
    let inputs = vec![image.copy()?, cleaned];
    let answer = run_once(networks, Role::Robust3, sizes, inputs)?;
    Halves::of(answer, image, count)?.joined()
}
