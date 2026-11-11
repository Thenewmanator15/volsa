//! Scroll bars that can be seen. egui's own float over what they scroll and are drawn only
//! while the pointer is over it, so a list cut short at the bottom of its window looks whole.
//! These take a strip of their own beside what they scroll, and are always drawn.

#[cfg(test)]
use egui::scroll_area::ScrollAreaOutput;
use egui::scroll_area::ScrollBarVisibility;
use egui::style::ScrollStyle;

/// Give the scroll areas made in `ui` from now on bars of their own that are always drawn
/// when there is more than can be seen, a little wider than egui's and with the handle in
/// the colour of the words, to stand out against the dark.
pub fn solid_bars(ui: &mut egui::Ui) {
    ui.spacing_mut().scroll = ScrollStyle {
        bar_width: 8.0,
        foreground_color: true,
        ..ScrollStyle::solid()
    };
}

/// An area that scrolls up and down, at most `max_height` points high, whose bar is drawn
/// however much it holds: a list that fits shows a bar with nothing to move.
pub fn up_and_down(max_height: f32) -> egui::ScrollArea {
    egui::ScrollArea::vertical()
        .max_height(max_height)
        .scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
}

/// What a scroll area held, and what of it could be seen.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scrolled {
    /// How high everything in it is, in points.
    pub held: f32,
    /// Where on the screen the part of it that can be seen is, its bar left out.
    pub shown: egui::Rect,
}

#[cfg(test)]
impl Scrolled {
    /// What `output` says of the scroll area that gave it, an area that shrinks to what it
    /// holds when that is smaller than the most it may take, as egui's do unless told not to.
    pub fn of<R>(output: &ScrollAreaOutput<R>) -> Self {
        let most = output.inner_rect;
        let taken = most.size().min(output.content_size);
        Self {
            held: output.content_size.y,
            shown: egui::Rect::from_min_size(most.min, taken),
        }
    }

    /// Whether some of what it holds is out of sight.
    pub fn is_cut(&self) -> bool {
        self.held > self.shown.height() + 0.5
    }
}

/// Whether a scroll bar is among `shapes`: a strip filled in a colour that can be seen,
/// beside the right edge of what can be seen of a scroll area, `shown`.
#[cfg(test)]
pub fn bar_drawn(shapes: &[egui::epaint::ClippedShape], shown: egui::Rect) -> bool {
    let beside = |rect: egui::Rect| {
        rect.left() >= shown.right()
            && rect.left() <= shown.right() + 20.0
            && rect.height() >= 0.5 * shown.height().min(40.0)
            && rect.top() < shown.bottom()
            && rect.bottom() > shown.top()
    };
    shapes.iter().any(|clipped| match &clipped.shape {
        egui::Shape::Rect(drawn) => drawn.fill.a() > 0 && beside(drawn.rect),
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use egui::{Pos2, RawInput, Rect, vec2};

    use super::*;

    /// A list of `lines` in an area at most 100 points high, and whether its bar is drawn.
    fn list_of(lines: usize, solid: bool) -> (Scrolled, bool) {
        let ctx = egui::Context::default();
        let mut scrolled = None;
        let mut drawn = false;
        for _ in 0..3 {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                ..RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                if solid {
                    solid_bars(ui);
                }
                let area = up_and_down(100.0).show(ui, |ui| {
                    for line in 0..lines {
                        ui.label(format!("Line {line}"));
                    }
                });
                scrolled = Some(Scrolled::of(&area));
            });
            // No renderer is behind these passes to take the font texture.
            output.textures_delta.clear();
            let shown = scrolled.map_or(Rect::NOTHING, |scrolled| scrolled.shown);
            drawn = bar_drawn(&output.shapes, shown);
        }
        (scrolled.expect("the area was laid out"), drawn)
    }

    #[test]
    fn a_long_list_is_cut_and_its_bar_is_drawn_without_the_pointer_over_it() {
        let (scrolled, drawn) = list_of(40, true);
        assert!(scrolled.is_cut(), "{scrolled:?}");
        assert!(drawn, "no bar is drawn beside {scrolled:?}");
        // egui's own bars are not drawn until the pointer is over them.
        let (scrolled, drawn) = list_of(40, false);
        assert!(scrolled.is_cut() && !drawn, "{scrolled:?}");
    }

    #[test]
    fn a_short_list_is_whole_and_shows_a_bar_with_nothing_to_move() {
        let (scrolled, drawn) = list_of(2, true);
        assert!(!scrolled.is_cut(), "{scrolled:?}");
        assert!(drawn, "no bar is drawn beside {scrolled:?}");
    }
}
