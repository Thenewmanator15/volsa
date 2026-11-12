//! How the middle of the window is shared out: the strip along its top that arranges the
//! pictures, and beneath it the pictures of the arrangement chosen.

use egui::{Rect, Ui, UiBuilder, pos2, vec2};

use super::viewport;
use crate::pictures::Pictures;

/// How high the strip above the pictures is, in points.
pub const STRIP: f32 = 24.0;

/// The middle of the window, where the pictures are: on the background, with the gap between
/// viewports around them.
pub fn middle() -> egui::Frame {
    egui::Frame::NONE
        .fill(super::theme::BACKGROUND)
        .inner_margin(viewport::GAP)
}

/// A part of `ui` for the middle of the window: all the room that is left but `wide` points
/// along the right, which a panel drawn after it takes. The Tab key goes through what is drawn
/// in the order it is drawn, so drawing the middle before the panel at its right puts the
/// pictures before the panel in the order of the keyboard, as they are laid out beside it.
pub fn beside_the_panel(ui: &mut Ui, wide: f32) -> Ui {
    let rest = ui.available_rect_before_wrap();
    let edge = (rest.right() - wide).max(rest.left());
    let beside = Rect::from_min_max(rest.min, pos2(edge, rest.bottom()));
    ui.new_child(UiBuilder::new().max_rect(beside))
}

/// The strip along the top of `area`, and what is left beneath it for the pictures, with the
/// gap between viewports between the two.
pub fn strip_and_pictures(area: Rect) -> (Rect, Rect) {
    let (strip, rest) = area.split_top_bottom_at_y(area.top() + STRIP.min(area.height()));
    (
        strip,
        rest.with_min_y((rest.top() + viewport::GAP).min(rest.bottom())),
    )
}

/// Where each kind of view is drawn this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Areas {
    /// The 3D scene, when it is shown.
    pub scene: Option<Rect>,
    /// The axial, coronal and sagittal planes, when they are shown.
    pub planes: Option<[Rect; 3]>,
    /// The 2D viewports, when they are shown.
    pub slices: Option<Rect>,
}

/// Give `area` to the pictures of an arrangement. The slices or the 3D view alone have it all;
/// the slices beside the 3D view have the left half and the 3D view the right; the planes
/// share it with the 3D view, two by two.
pub fn areas(pictures: Pictures, area: Rect) -> Areas {
    let nothing = Areas {
        scene: None,
        planes: None,
        slices: None,
    };
    match pictures {
        Pictures::Slices => Areas {
            slices: Some(area),
            ..nothing
        },
        Pictures::SlicesAndScene => {
            let half = (area.width() - viewport::GAP) / 2.0;
            let left = Rect::from_min_size(area.min, vec2(half, area.height()));
            let right = Rect::from_min_max(pos2(area.right() - half, area.top()), area.max);
            Areas {
                slices: Some(left),
                scene: Some(right),
                ..nothing
            }
        }
        Pictures::Planes => {
            let [axial, sagittal, coronal, scene] = quad(area);
            Areas {
                scene: Some(scene),
                planes: Some([axial, coronal, sagittal]),
                ..nothing
            }
        }
        Pictures::Scene => Areas {
            scene: Some(area),
            ..nothing
        },
    }
}

/// Four panels of one size, two by two, with the viewports' gap between them: top left, top
/// right, bottom left, bottom right. The planes are axial, sagittal, coronal and then the 3D
/// scene, the arrangement most viewers use.
pub fn quad(area: Rect) -> [Rect; 4] {
    let cell = vec2(
        (area.width() - viewport::GAP) / 2.0,
        (area.height() - viewport::GAP) / 2.0,
    );
    let place = |col: f32, row: f32| {
        let min = area.min
            + vec2(
                col * (cell.x + viewport::GAP),
                row * (cell.y + viewport::GAP),
            );
        Rect::from_min_size(min, cell)
    };
    [
        place(0.0, 0.0),
        place(1.0, 0.0),
        place(0.0, 1.0),
        place(1.0, 1.0),
    ]
}

#[cfg(test)]
mod tests;
