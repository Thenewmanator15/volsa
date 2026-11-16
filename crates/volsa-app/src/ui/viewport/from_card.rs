//! A slice drawn from the scan's volume where the graphics card already holds it, as the plane
//! views draw theirs: no slice is decoded, copied or uploaded to show it, so stepping through
//! the slices, or playing them, costs a few numbers a frame. Only when the volume on the card
//! is the scan shown, whole: a scan in colour, of several volumes, halved to fit the card, or
//! another series than the scene's is drawn from its own slices as before.

use egui_wgpu::RenderState;
use volsa_core::{Patient, Point, Voxel};
use volsa_render::{LabelsOnDevice, VolumePipeline};

use super::{Pane, pixel_aspect};
use crate::labels::on_study;
use crate::ui::planes::PlaneGpu;

/// Draw the slice viewport `pane` shows from `volume`, the scene's volume on the card, with
/// the labels open drawn over it when they are of its study. False, with nothing drawn, when
/// it cannot be drawn so.
pub(super) fn draw(
    ui: &egui::Ui,
    pane: &Pane<'_>,
    (volume, labels): (Option<&VolumePipeline>, Option<&LabelsOnDevice>),
    pipeline: &PlaneGpu,
    render: &RenderState,
) -> bool {
    let state = pane.state;
    let Some(view) = state.viewports.get(pane.at) else {
        return false;
    };
    let (Some(series), Some(volume)) = (view.series.as_ref(), volume) else {
        return false;
    };
    let Some((info, _)) = pane.series() else {
        return false;
    };
    let one_grey_volume = !info.colour && info.set.is_none();
    if state.scene.series.as_ref() != Some(series) || !one_grey_volume {
        return false;
    }
    let (Some(place), Ok(slices), Ok(depth)) = (
        info.voxel_to_patient.as_ref(),
        u32::try_from(state.slices_in(pane.at)),
        u32::try_from(view.slice),
    ) else {
        return false;
    };
    let image = [info.cols, info.rows];
    let size = [pane.cell.width(), pane.cell.height()];
    let to_screen = pane
        .shown
        .image_to_screen(image, size, pixel_aspect(state, pane.at));
    let Some(to_image) = to_screen.inverse() else {
        return false;
    };
    // The point of the patient a pixel of the viewport shows: through the image the 2D view
    // lays out, at the depth of the slice.
    let at = |x: f64, y: f64| {
        let pixel = to_image.apply(Point::new(x, y, 0.0));
        place.apply(Point::<Voxel>::new(pixel.x, pixel.y, f64::from(depth)))
    };
    let corner = at(0.0, 0.0);
    let step = |to: Point<Patient>| [to.x - corner.x, to.y - corner.y, to.z - corner.z];
    let sides = [step(at(1.0, 0.0)), step(at(0.0, 1.0))];
    let labels = labels.filter(|_| on_study(state, series));
    pipeline.labels(labels, state.labels.overlay);
    let [cols, rows] = image;
    pipeline.draw_slice(
        (ui, pane.cell),
        (render, volume),
        [cols, rows, slices],
        (corner, sides),
        view.wl,
    )
}
