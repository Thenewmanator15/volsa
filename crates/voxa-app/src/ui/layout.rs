//! How the 3D scene and the 2D viewports share the middle of the window: one has the large
//! part, the other a column beside it, and a button swaps them.

use egui::{Align, Layout, Rect, RichText, Ui, UiBuilder, vec2};

use super::{theme, viewport};
use crate::action::Action;
use crate::planes::ColumnShows;
use crate::scene::MainPanel;

/// The least share of the width the column takes, however far it is dragged.
const LEAST_SHARE: f32 = 0.15;
/// The most of the width the column may take, so that the large part keeps two fifths.
const MOST_SHARE: f32 = 0.6;
/// The least the column is given while that is no more than half the width: what the
/// strip along its top needs for two tabs and a button.
const LEAST_COLUMN: f32 = 260.0;
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

/// The width of the column in an area `width` across, for its `share` of it: between 15%
/// and 60% of the width, but not under 260 points while that is no more than half of it.
pub fn column_width(width: f32, share: f32) -> f32 {
    let least = LEAST_COLUMN.min(width / 2.0);
    (width * share.clamp(LEAST_SHARE, MOST_SHARE)).max(least)
}

/// The column's share of an area `width` across once its left edge has been dragged `by`
/// points from where its `share` puts it, as far as its limits allow.
pub fn column_dragged(width: f32, share: f32, by: f32) -> f32 {
    let wanted = column_width(width, share) - by;
    column_width(width, wanted / width) / width
}

/// Divide an area into the large part and the column beside it, which takes its `share` of
/// the width as [`column_width`] has it, with the gap between viewports between them.
pub fn panels(area: Rect, share: f32) -> Panels {
    let column = column_width(area.width(), share);
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

/// Say what the column shows, as tabs to choose between where there is a choice: the slices
/// or the planes with the 3D scene large. With the slices large the column holds the scene,
/// and with the planes large it holds the slices; there is nothing to choose then.
fn shown_in_the_column(
    ui: &mut Ui,
    large: MainPanel,
    column: ColumnShows,
    actions: &mut Vec<Action>,
) {
    let slices = (ColumnShows::Slices, "Slices", "The series' own slices");
    let planes = (
        ColumnShows::Planes,
        "Planes",
        "Axial, coronal and sagittal planes through one point",
    );
    let alone = match large {
        MainPanel::Stack => Some("3D"),
        MainPanel::Planes => Some("Slices"),
        MainPanel::Scene => None,
    };
    if let Some(name) = alone {
        ui.label(RichText::new(name).small().color(theme::QUIET_TEXT));
        return;
    }
    for (shows, name, hover) in [slices, planes] {
        let tab = ui.selectable_label(column == shows, RichText::new(name).small());
        if tab.on_hover_text(hover).clicked() && column != shows {
            actions.push(Action::SetColumn(shows));
        }
    }
}

#[cfg(test)]
mod tests;
