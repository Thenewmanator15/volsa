//! How the 3D scene and the 2D viewports share the middle of the window: one has the large
//! part, the other a column beside it, and a button swaps them.

use egui::{Align, Layout, Rect, RichText, Ui, UiBuilder, vec2};

use super::{theme, viewport};
use crate::action::Action;
use crate::planes::ColumnShows;
use crate::scene::MainPanel;

/// The width of the column beside the main area, in points, where there is room for it.
const SIDE_COLUMN: f32 = 360.0;
/// The most of the width the column may take.
const SIDE_SHARE: f32 = 1.0 / 3.0;
/// The height of the strip along the top of the column, which says what the column shows and
/// holds the button that enlarges it.
const HEADER: f32 = 24.0;

/// The two areas the middle of the window is divided into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panels {
    /// The large part, at the left.
    pub main: Rect,
    /// The column at the right.
    pub side: Rect,
}

/// Divide an area into the large part and the column beside it: the column is 360 points
/// wide, or a third of the area if that is less, with the gap between viewports between them.
pub fn panels(area: Rect) -> Panels {
    let column = SIDE_COLUMN.min(area.width() * SIDE_SHARE);
    let (main, side) = area.split_left_right_at_x(area.right() - column);
    Panels {
        main: main.with_max_x(main.right() - viewport::GAP),
        side,
    }
}

/// The strip along the top of the column, and what is left of the column beneath it.
pub fn header_and_rest(side: Rect) -> (Rect, Rect) {
    let (header, rest) = side.split_top_bottom_at_y(side.top() + HEADER.min(side.height()));
    (
        header,
        rest.with_min_y((rest.top() + viewport::GAP).min(rest.bottom())),
    )
}

/// Where each kind of view is drawn this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Areas {
    /// The 3D scene, which is always somewhere.
    pub scene: Rect,
    /// The axial, coronal and sagittal planes, when they are shown.
    pub planes: Option<[Rect; 3]>,
    /// The 2D viewports, when they are shown.
    pub slices: Option<Rect>,
}

/// Give the large part and the column to what is shown. With the 3D scene large the column
/// holds the slices or the three planes, one above another; with the slices large it holds
/// the scene; with the planes large they share the large part with the scene, two by two,
/// and the column holds the slices.
pub fn areas(large: MainPanel, column: ColumnShows, main: Rect, side: Rect) -> Areas {
    match (large, column) {
        (MainPanel::Scene, ColumnShows::Slices) => Areas {
            scene: main,
            planes: None,
            slices: Some(side),
        },
        (MainPanel::Scene, ColumnShows::Planes) => Areas {
            scene: main,
            planes: Some(stacked(side)),
            slices: None,
        },
        (MainPanel::Stack, _) => Areas {
            scene: side,
            planes: None,
            slices: Some(main),
        },
        (MainPanel::Planes, _) => {
            let [axial, sagittal, coronal, scene] = quad(main);
            Areas {
                scene,
                planes: Some([axial, coronal, sagittal]),
                slices: Some(side),
            }
        }
    }
}

/// Four panels of one size, two by two, with the viewports' gap between them: top left, top
/// right, bottom left, bottom right. The planes large are axial, sagittal, coronal and then
/// the 3D scene, the arrangement most viewers use.
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

/// Three panels of one size, one above another, with the viewports' gap between them.
pub fn stacked(area: Rect) -> [Rect; 3] {
    let high = (area.height() - 2.0 * viewport::GAP) / 3.0;
    let place = |row: f32| {
        let min = area.min + vec2(0.0, row * (high + viewport::GAP));
        Rect::from_min_size(min, vec2(area.width(), high))
    };
    [place(0.0), place(1.0), place(2.0)]
}

/// Draw the strip along the top of the column: what the column shows, and a button that gives
/// it the large part of the window. `large` is what has the large part now. With the 3D
/// scene large the strip has a tab for the slices and one for the planes, and the button
/// enlarges whichever is shown. Without a 3D view (`there_is_3d` false) the button cannot
/// give the 3D scene the large part.
pub fn header(
    ui: &mut Ui,
    strip: Rect,
    (large, column): (MainPanel, ColumnShows),
    there_is_3d: bool,
    actions: &mut Vec<Action>,
) {
    let enlarged = match (large, column) {
        (MainPanel::Scene, ColumnShows::Planes) => MainPanel::Planes,
        (MainPanel::Scene | MainPanel::Planes, _) => MainPanel::Stack,
        (MainPanel::Stack, _) => MainPanel::Scene,
    };
    let inside = strip.shrink2(vec2(4.0, 0.0));
    let mut strip = ui.new_child(UiBuilder::new().max_rect(inside));
    strip.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let can = there_is_3d || large != MainPanel::Stack;
        let enlarge = ui.add_enabled(can, egui::Button::new("Enlarge").small());
        let enlarge = enlarge.on_disabled_hover_text("This series has no 3D view to enlarge");
        if enlarge
            .on_hover_text("Give this the large part of the window")
            .clicked()
        {
            actions.push(Action::SetMain(enlarged));
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            shown_in_the_column(ui, large, column, actions);
        });
    });
}

/// Say what the column shows; with the 3D scene large, as two tabs to choose between.
fn shown_in_the_column(
    ui: &mut Ui,
    large: MainPanel,
    column: ColumnShows,
    actions: &mut Vec<Action>,
) {
    let quiet = |text: &str| RichText::new(text).small().color(theme::QUIET_TEXT);
    match large {
        MainPanel::Stack => drop(ui.label(quiet("3D"))),
        MainPanel::Planes => drop(ui.label(quiet("Slices"))),
        MainPanel::Scene => {
            let tabs = [
                (ColumnShows::Slices, "Slices", "The series' own slices"),
                (
                    ColumnShows::Planes,
                    "Planes",
                    "Axial, coronal and sagittal planes through one point",
                ),
            ];
            for (shows, name, hover) in tabs {
                let tab = ui.selectable_label(column == shows, RichText::new(name).small());
                if tab.on_hover_text(hover).clicked() && column != shows {
                    actions.push(Action::SetColumn(shows));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
    fn in_a_narrow_area_the_column_is_a_third_of_it() {
        let Panels { main, side } = panels(area(600.0, 400.0));
        assert_eq!(side.width(), 200.0);
        assert_eq!(main.width(), 400.0 - viewport::GAP);
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
    fn with_the_scene_large_the_strip_has_a_tab_for_slices_and_one_for_planes() {
        let (slices, planes) = (ColumnShows::Slices, ColumnShows::Planes);
        assert_eq!(
            sweep(MainPanel::Scene, slices, true),
            [Action::SetColumn(planes), Action::SetMain(MainPanel::Stack)],
            "the tab not shown, then Enlarge for the slices"
        );
        assert_eq!(
            sweep(MainPanel::Scene, planes, true),
            [
                Action::SetColumn(slices),
                Action::SetMain(MainPanel::Planes)
            ],
            "Enlarge gives the planes the large part"
        );
    }

    #[test]
    fn the_button_on_the_column_gives_what_it_shows_the_large_part() {
        let slices = ColumnShows::Slices;
        assert_eq!(
            sweep(MainPanel::Stack, slices, true),
            [Action::SetMain(MainPanel::Scene)]
        );
        assert_eq!(
            sweep(MainPanel::Planes, slices, true),
            [Action::SetMain(MainPanel::Stack)]
        );
        assert_eq!(
            sweep(MainPanel::Planes, ColumnShows::Planes, true),
            [Action::SetMain(MainPanel::Stack)],
            "with the planes large the column holds the slices whatever it held before"
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
}
