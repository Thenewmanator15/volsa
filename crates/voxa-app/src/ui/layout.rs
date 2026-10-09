//! How the 3D scene and the 2D viewports share the middle of the window: one has the large
//! part, the other a column beside it, and a button swaps them.

use egui::{Align, Layout, Rect, RichText, Ui, UiBuilder, vec2};

use super::{theme, viewport};
use crate::action::Action;
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

/// Draw the strip along the top of the column: what the column shows, and a button that gives
/// it the large part of the window. `large` is what has the large part now.
pub fn header(ui: &mut Ui, strip: Rect, large: MainPanel, actions: &mut Vec<Action>) {
    let (shows, enlarged) = match large {
        MainPanel::Scene => ("Slices", MainPanel::Stack),
        MainPanel::Stack => ("3D", MainPanel::Scene),
    };
    let inside = strip.shrink2(vec2(4.0, 0.0));
    let mut strip = ui.new_child(UiBuilder::new().max_rect(inside));
    strip.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let enlarge = ui.small_button("Enlarge");
        if enlarge
            .on_hover_text("Give this the large part of the window")
            .clicked()
        {
            actions.push(Action::SetMain(enlarged));
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.label(RichText::new(shows).small().color(theme::QUIET_TEXT));
        });
    });
}

#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, pos2};

    use super::*;

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

    /// Click all the way along the strip and gather what was asked for.
    fn sweep(large: MainPanel) -> Vec<Action> {
        let ctx = egui::Context::default();
        let strip = Rect::from_min_size(pos2(0.0, 0.0), vec2(360.0, 24.0));
        let mut asked = Vec::new();
        let mut pass = |events: Vec<Event>| {
            let input = RawInput {
                events,
                ..RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| header(ui, strip, large, &mut asked));
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
    fn the_button_on_the_column_gives_what_it_shows_the_large_part() {
        assert_eq!(sweep(MainPanel::Scene), [Action::SetMain(MainPanel::Stack)]);
        assert_eq!(sweep(MainPanel::Stack), [Action::SetMain(MainPanel::Scene)]);
    }
}
