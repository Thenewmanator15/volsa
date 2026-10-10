//! How the window is shared out, and what the strip above the column offers.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, pos2};

use super::*;
use crate::planes::ColumnShows;

fn area(width: f32, height: f32) -> Rect {
    Rect::from_min_size(pos2(100.0, 50.0), vec2(width, height))
}

#[test]
fn the_column_is_360_points_wide_at_the_right_with_a_gap_before_it() {
    let Panels { main, side } = panels(area(1200.0, 700.0));
    assert_eq!(
        (side.left(), side.right(), side.width()),
        (940.0, 1300.0, 360.0)
    );
    assert_eq!((main.left(), main.right()), (100.0, 940.0 - viewport::GAP));
    assert_eq!((main.height(), side.height()), (700.0, 700.0));
}

#[test]
fn in_a_narrow_area_the_column_is_a_third_of_it_and_no_narrower_than_its_tabs() {
    let Panels { main, side } = panels(area(900.0, 400.0));
    assert_eq!(side.width(), 300.0, "a third");
    assert_eq!(main.width(), 600.0 - viewport::GAP);
    let Panels { main, side } = panels(area(600.0, 400.0));
    assert_eq!(
        side.width(),
        260.0,
        "a third would not hold the strip's three tabs"
    );
    assert_eq!(main.width(), 340.0 - viewport::GAP);
    let Panels { side, .. } = panels(area(300.0, 400.0));
    assert_eq!(side.width(), 150.0, "never more than half");
}

#[test]
fn the_column_has_a_strip_along_its_top() {
    let (strip, rest) = header_and_rest(area(360.0, 700.0));
    assert_eq!((strip.top(), strip.height()), (50.0, 24.0));
    assert_eq!((rest.top(), rest.bottom()), (74.0 + viewport::GAP, 750.0));
    let (strip, rest) = header_and_rest(area(360.0, 10.0));
    assert!(
        strip.height() <= 10.0 && rest.height() >= 0.0,
        "nothing is turned inside out"
    );
}

/// Click all the way along the strip and gather what was asked for, from left to right.
fn sweep(large: MainPanel, column: ColumnShows, there_is_3d: bool) -> Vec<Action> {
    let ctx = egui::Context::default();
    let strip = Rect::from_min_size(pos2(0.0, 0.0), vec2(360.0, 24.0));
    let mut asked = Vec::new();
    let mut pass = |events: Vec<Event>| {
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            header(ui, strip, (large, column), there_is_3d, &mut asked);
        });
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
    };
    pass(Vec::new());
    for x in (2..360u16).step_by(4) {
        let pos = Pos2::new(f32::from(x), 12.0);
        let button = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        pass(vec![Event::PointerMoved(pos), button(true)]);
        pass(vec![button(false)]);
    }
    asked.dedup();
    asked
}

#[test]
fn with_the_scene_large_the_strip_has_a_tab_for_slices_planes_and_parts() {
    let (slices, planes) = (ColumnShows::Slices, ColumnShows::Planes);
    let parts = Action::SetColumn(ColumnShows::Parts);
    assert_eq!(
        sweep(MainPanel::Scene, slices, true),
        [
            Action::SetColumn(planes),
            parts.clone(),
            Action::SetMain(MainPanel::Stack)
        ],
        "the tabs not shown, then Enlarge for the slices"
    );
    assert_eq!(
        sweep(MainPanel::Scene, planes, true),
        [
            Action::SetColumn(slices),
            parts,
            Action::SetMain(MainPanel::Planes)
        ],
        "Enlarge gives the planes the large part"
    );
    assert_eq!(
        sweep(MainPanel::Scene, ColumnShows::Parts, true),
        [Action::SetColumn(slices), Action::SetColumn(planes)],
        "a list has nothing to enlarge"
    );
}

#[test]
fn the_button_on_the_column_gives_what_it_shows_the_large_part() {
    let slices = ColumnShows::Slices;
    assert_eq!(
        sweep(MainPanel::Stack, slices, true),
        [Action::SetMain(MainPanel::Scene)]
    );
    let parts = Action::SetColumn(ColumnShows::Parts);
    assert_eq!(
        sweep(MainPanel::Planes, slices, true),
        [parts.clone(), Action::SetMain(MainPanel::Stack)]
    );
    assert_eq!(
        sweep(MainPanel::Planes, ColumnShows::Planes, true),
        [parts, Action::SetMain(MainPanel::Stack)],
        "with the planes large the column holds the slices, or the parts"
    );
    assert_eq!(
        sweep(MainPanel::Planes, ColumnShows::Parts, true),
        [Action::SetColumn(slices)]
    );
}

#[test]
fn with_no_3d_view_there_is_nothing_for_the_button_to_enlarge() {
    assert_eq!(sweep(MainPanel::Stack, ColumnShows::Slices, false), []);
}

#[test]
fn four_panels_share_the_large_part_two_by_two() {
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
fn three_panels_share_the_column_one_above_another() {
    let [first, second, third] = stacked(area(360.0, 700.0));
    let high = (700.0 - 2.0 * viewport::GAP) / 3.0;
    assert!(
        [first, second, third]
            .iter()
            .all(|panel| panel.width() == 360.0)
    );
    assert!((first.height() - high).abs() < 1.0e-3 && (third.bottom() - 750.0).abs() < 1.0e-3);
    assert!((second.top() - first.bottom() - viewport::GAP).abs() < 1.0e-3);
}

#[test]
fn each_kind_of_view_is_given_its_part_of_the_window() {
    let (main, side) = (area(900.0, 700.0), area(360.0, 700.0));
    let (slices, planes) = (ColumnShows::Slices, ColumnShows::Planes);
    let scene_large = areas(MainPanel::Scene, slices, main, side);
    assert_eq!(
        (scene_large.scene, scene_large.slices, scene_large.planes),
        (main, Some(side), None)
    );
    let with_planes = areas(MainPanel::Scene, planes, main, side);
    assert_eq!((with_planes.scene, with_planes.slices), (main, None));
    assert_eq!(with_planes.planes, Some(stacked(side)));
    let listed = areas(MainPanel::Scene, ColumnShows::Parts, main, side);
    assert_eq!((listed.scene, listed.parts), (main, Some(side)));
    assert_eq!((listed.slices, listed.planes), (None, None));
    let beside_the_planes = areas(MainPanel::Planes, ColumnShows::Parts, main, side);
    assert_eq!(
        (beside_the_planes.parts, beside_the_planes.slices),
        (Some(side), None)
    );
    assert_eq!(
        areas(MainPanel::Stack, ColumnShows::Parts, main, side).parts,
        None
    );
    for column in [slices, planes] {
        let stack_large = areas(MainPanel::Stack, column, main, side);
        assert_eq!(
            (stack_large.scene, stack_large.slices, stack_large.planes),
            (side, Some(main), None)
        );
        let planes_large = areas(MainPanel::Planes, column, main, side);
        let [top_left, top_right, bottom_left, bottom_right] = quad(main);
        // Axial, coronal, sagittal: axial top left, sagittal beside it, coronal beneath.
        assert_eq!(
            planes_large.planes,
            Some([top_left, bottom_left, top_right])
        );
        assert_eq!(
            (planes_large.scene, planes_large.slices),
            (bottom_right, Some(side))
        );
    }
}
