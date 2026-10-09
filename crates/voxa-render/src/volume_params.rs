//! The numbers the volume shader draws one picture from, worked out from the scene and the
//! volume on the graphics device.

use bytemuck::{Pod, Zeroable};
use voxa_core::{Patient, Point};

use crate::scene::{Look, MAX_CUTS, SceneParams};
use crate::vector::{dot, unit};
use crate::volume_texture::{BLOCK, Placed};

/// How much of the light a millimetre of the brightest tissue stops in the see-through look.
const VR_STOPPED_PER_MM: f32 = 0.05;
/// How much of the light a millimetre of solid tissue stops in the surface look: all but a
/// fiftieth of it within 4 mm, so the first tissue met hides what is behind it.
const SURFACE_STOPPED_PER_MM: f32 = 1.0;
/// Where tissue begins to stop light in the surface look, as a share of the window up from
/// its low end. A scan's air is never quite empty; what is fainter than this is left out, so
/// that the air is not drawn as a fog in front of the patient.
pub(crate) const SURFACE_BEGINS: f32 = 0.1;
/// The share of the window over which tissue then becomes solid in the surface look. In the
/// see-through look tissue begins at the window's low end and takes the whole window.
pub(crate) const SURFACE_RAMP: f32 = 0.1;
/// The coarsest a picture is drawn: steps ten times as long as at full quality.
const MIN_QUALITY: f32 = 0.1;
/// The narrowest window the shader is given, as a share of the volume's range of values.
const MIN_WINDOW: f32 = 1.0e-6;

/// What the shader needs besides the volume; the layout matches `Scene` in `volume.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct Params {
    across: [f32; 4],
    along: [f32; 4],
    forward: [f32; 4],
    top_left: [f32; 4],
    to_u: [f32; 4],
    to_v: [f32; 4],
    to_w: [f32; 4],
    view: [f32; 4],
    window: [f32; 4],
    voxel: [f32; 4],
    cuts: [[f32; 4]; MAX_CUTS],
    sheet: [f32; 4],
    planes: [f32; 4],
    grid: [f32; 4],
    ramp: [f32; 4],
}

/// The numbers the shader draws one picture from.
pub(crate) fn shader_params(placed: &Placed, scene: &SceneParams, viewport: [f32; 2]) -> Params {
    let to_patient = scene.camera.screen_to_patient(viewport);
    let [across, along, forward] = to_patient.columns().map(four);
    // How far along one axis of the volume a point of the patient is, from 0 to 1: its
    // voxel coordinate, counted from the face half a voxel before the first voxel's centre.
    let to_voxel = placed.patient_to_voxel;
    let along_axis = |pick: fn([f64; 3]) -> f64, count: u32| {
        let [x, y, z] = to_voxel.columns().map(pick);
        let from_face = pick(to_voxel.origin()) + 0.5;
        [x, y, z, from_face].map(|part| single(part / f64::from(count)))
    };
    let [cols, rows, slices] = placed.dims;
    let quality = if scene.quality.is_finite() {
        scene.quality
    } else {
        1.0
    };
    let step_mm = single(placed.finest_mm / 2.0) / quality.clamp(MIN_QUALITY, 1.0);
    // The look as the shader knows it, how much light a millimetre stops, and where in the
    // window tissue begins to stop it and over how much it becomes solid.
    let (look, stopped_per_mm, ramp) = match scene.look {
        Look::Vr => (0.0, VR_STOPPED_PER_MM, [0.0, 1.0]),
        Look::Mip => (1.0, 0.0, [0.0, 1.0]),
        Look::Surface => (2.0, SURFACE_STOPPED_PER_MM, [SURFACE_BEGINS, SURFACE_RAMP]),
    };
    let [begins, becomes_solid_over] = ramp;
    let (cuts, cuts_used) = cuts_in_use(scene);
    let sheet = scene
        .slice
        .and_then(|slice| plane(slice.point, slice.normal));
    // What is stepped over is too faint to draw in the look in use. In the brightest-value look
    // that is anything no brighter than the bottom of the window, which is black whatever else
    // lies on the line.
    let skipping = scene.skip_empty;
    let [wide, high] = viewport;
    let black = (scene.window.low() - placed.smallest) / placed.span;
    let black_to_white = (scene.window.width / placed.span).max(MIN_WINDOW);
    Params {
        across,
        along,
        forward,
        top_left: four(to_patient.origin()),
        to_u: along_axis(|[col, _, _]| col, cols),
        to_v: along_axis(|[_, row, _]| row, rows),
        to_w: along_axis(|[_, _, slice]| slice, slices),
        view: [wide, high, step_mm, look],
        window: [black, black_to_white, stopped_per_mm, 0.0],
        ramp: [begins, becomes_solid_over, 0.0, 0.0],
        voxel: [cols, rows, slices, 1].map(|count| 1.0 / count as f32),
        cuts,
        sheet: sheet.unwrap_or_default(),
        planes: [cuts_used, flag(sheet.is_some()), flag(skipping), 0.0],
        grid: [cols as f32, rows as f32, slices as f32, 0.0].map(|count| count / BLOCK as f32),
    }
}

/// The cut planes as the shader keeps them, and how many there are. Only the first few
/// count, and of those only the ones that have a direction.
fn cuts_in_use(scene: &SceneParams) -> ([[f32; 4]; MAX_CUTS], f32) {
    let mut cuts = [[0.0; 4]; MAX_CUTS];
    let mut used = 0.0;
    let given = scene.cuts.iter().take(MAX_CUTS);
    let usable = given.filter_map(|cut| plane(cut.point, cut.normal));
    for (kept, cut) in cuts.iter_mut().zip(usable) {
        *kept = cut;
        used += 1.0;
    }
    (cuts, used)
}

/// A plane as the shader keeps it: its direction with a length of 1, then how far along that
/// direction the plane lies from the patient origin. `None` for a plane with no direction.
fn plane(point: Point<Patient>, normal: [f64; 3]) -> Option<[f32; 4]> {
    let [x, y, z] = unit(normal)?;
    let from_origin = dot([x, y, z], [point.x, point.y, point.z]);
    from_origin
        .is_finite()
        .then(|| [x, y, z, from_origin].map(single))
}

/// Yes or no as the number the shader keeps it in.
fn flag(yes: bool) -> f32 {
    if yes { 1.0 } else { 0.0 }
}

/// Three numbers as the four the shader keeps them in.
fn four([x, y, z]: [f64; 3]) -> [f32; 4] {
    [single(x), single(y), single(z), 0.0]
}

/// A length or coordinate as the single-precision number the shader works in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "millimetres and pixels are small"
)]
fn single(value: f64) -> f32 {
    value as f32
}
