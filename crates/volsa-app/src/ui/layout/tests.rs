//! How the middle of the window is shared out among the pictures of each arrangement.

use egui::pos2;

use super::*;
use crate::pictures::Pictures;

fn area(width: f32, height: f32) -> Rect {
    Rect::from_min_size(pos2(100.0, 50.0), vec2(width, height))
}

#[test]
fn each_arrangement_gives_its_areas() {
    let whole = area(1000.0, 800.0);
    let slices = areas(Pictures::Slices, whole);
    assert_eq!(
        (slices.slices, slices.scene, slices.planes),
        (Some(whole), None, None),
        "the slices have it all"
    );
    let both = areas(Pictures::SlicesAndScene, whole);
    let half = (1000.0 - viewport::GAP) / 2.0;
    let left = Rect::from_min_size(pos2(100.0, 50.0), vec2(half, 800.0));
    let right = Rect::from_min_max(pos2(1100.0 - half, 50.0), pos2(1100.0, 850.0));
    assert_eq!(
        (both.slices, both.scene, both.planes),
        (Some(left), Some(right), None),
        "the slices at the left, the 3D view at the right"
    );
    assert_eq!(right.left() - left.right(), viewport::GAP);
    let planes = areas(Pictures::Planes, whole);
    let [top_left, top_right, bottom_left, bottom_right] = quad(whole);
    // Axial, coronal, sagittal: axial top left, sagittal beside it, coronal beneath.
    assert_eq!(planes.planes, Some([top_left, bottom_left, top_right]));
    assert_eq!((planes.scene, planes.slices), (Some(bottom_right), None));
    let scene = areas(Pictures::Scene, whole);
    assert_eq!(
        (scene.scene, scene.slices, scene.planes),
        (Some(whole), None, None),
        "the 3D view has it all"
    );
}

#[test]
fn four_panels_share_the_pictures_two_by_two() {
    let [top_left, top_right, bottom_left, bottom_right] = quad(area(1000.0, 800.0));
    let size = vec2(
        (1000.0 - viewport::GAP) / 2.0,
        (800.0 - viewport::GAP) / 2.0,
    );
    for panel in [top_left, top_right, bottom_left, bottom_right] {
        assert_eq!(panel.size(), size);
    }
    assert_eq!(top_left.min, pos2(100.0, 50.0));
    assert_eq!(top_right.left(), top_left.right() + viewport::GAP);
    assert_eq!(bottom_left.top(), top_left.bottom() + viewport::GAP);
    assert_eq!(bottom_right.max, pos2(1100.0, 850.0));
}

#[test]
fn the_strip_runs_along_the_top_of_the_pictures() {
    let (strip, rest) = strip_and_pictures(area(900.0, 700.0));
    assert_eq!(
        (strip.top(), strip.height(), strip.width()),
        (50.0, 24.0, 900.0)
    );
    assert_eq!((rest.top(), rest.bottom()), (74.0 + viewport::GAP, 750.0));
    assert_eq!(rest.x_range(), strip.x_range());
    let (strip, rest) = strip_and_pictures(area(900.0, 10.0));
    assert!(
        strip.height() <= 10.0 && rest.height() >= 0.0,
        "nothing is turned inside out"
    );
}
