//! The annotations a picture of the window can ask for, made as the tools would have made
//! them.

use voxa_core::{HeldVolume, Patient, Point};
use voxa_volume::{PlaneKind, ellipse_stats};

use crate::action::Action;
use crate::annotations::Shape;
use crate::planes::plane;
use crate::state::AppState;

/// For a picture that asks for it with the word `measured`: a ruler, an angle, a region and
/// a marker in the axial plane through the position, as the tools would have made them from
/// `scan`, the scene's volume. Nothing while there is no volume ready.
pub fn sample_annotations(state: &AppState, scan: &HeldVolume) -> Vec<Action> {
    let Some((axial, corners)) = plane(state, PlaneKind::Axial).zip(state.scene.corners) else {
        return Vec::new();
    };
    // A fifth of how far the volume reaches from the middle of the plane, as the unit.
    let reach = corners.iter().fold(0.0_f64, |reach, corner| {
        let ([across, down], _) = axial.of(*corner);
        reach.max(across.abs()).max(down.abs())
    });
    let unit = reach / 5.0;
    let at = |across: f64, down: f64| xyz(axial.at([across * unit, down * unit]));
    let radii_mm = [unit, 0.6 * unit];
    let inside = ellipse_stats(scan, &axial, [0.0, -2.6 * unit], radii_mm);
    let shapes = [
        Shape::Ruler {
            a: at(-2.0, -1.0),
            b: at(2.0, -1.0),
        },
        Shape::Angle {
            a: at(2.0, 2.5),
            vertex: at(0.0, 0.8),
            b: at(-2.0, 2.5),
        },
        Shape::Region {
            centre: at(0.0, -2.6),
            across: axial.across,
            down: axial.down,
            radii_mm,
            stats: inside,
        },
        Shape::Marker { at: at(3.0, 0.5) },
    ];
    shapes.into_iter().map(Action::AnnotationAdd).collect()
}

fn xyz(point: Point<Patient>) -> [f64; 3] {
    [point.x, point.y, point.z]
}
