//! Sliders beside the numbers of the View tab: brightness and contrast for the window of the
//! active viewport, and the speed of playback. Each moves the same setting as the number
//! beside it, so either can be used.

use egui::{Slider, SliderClamping, Ui};
use volsa_core::WindowLevel;

use super::super::super::access;
use crate::state::{AppState, Cine};

/// What the brightness slider says when pointed at.
pub(super) const BRIGHTNESS_SAYS: &str = "Slide right for a brighter picture";

/// What the contrast slider says when pointed at.
pub(super) const CONTRAST_SAYS: &str = "Slide right for more contrast";

/// How far the sliders reach for one picture: the lowest and highest level, and the widest
/// width. Worked out from the Soft window of the slice shown, which spans nearly all its
/// values, with half as much again on either side; before the slice has given its windows,
/// from the window shown. Always wide enough to hold the window shown, so a window typed
/// beyond the reach is never pulled back by the slider.
pub(super) fn reach(around: WindowLevel, shown: WindowLevel) -> (f32, f32, f32) {
    let span = around.width.max(1.0);
    let low = around.center - around.width / 2.0 - span / 2.0;
    let high = around.center + around.width / 2.0 + span / 2.0;
    (
        low.min(shown.center),
        high.max(shown.center),
        (span * 2.0).max(shown.width),
    )
}

/// The window the reach of the sliders is worked out from: the Soft window of the slice the
/// active viewport shows, once it is known, or else the window shown.
fn around(state: &AppState, shown: WindowLevel) -> WindowLevel {
    let viewport = state.active;
    let soft = state.viewports.get(viewport).and_then(|view| {
        let series = view.series.clone()?;
        state
            .windows
            .get(&(series, view.slice))
            .map(|windows| windows[1])
    });
    soft.unwrap_or(shown)
}

/// Draw the brightness and contrast sliders for the window shown; the window they were moved
/// to, if they were.
pub(super) fn window(ui: &mut Ui, state: &AppState, shown: WindowLevel) -> Option<WindowLevel> {
    let (low, high, widest) = reach(around(state, shown), shown);
    let (mut level, mut width) = (shown.center, shown.width);
    // A higher level draws the picture darker and a wider window flatter: both run backwards,
    // so that sliding right is brighter and sharper.
    let brightness = Slider::new(&mut level, high..=low)
        .text("Brightness")
        .show_value(false)
        .clamping(SliderClamping::Never);
    let contrast = Slider::new(&mut width, widest..=1.0)
        .text("Contrast")
        .show_value(false)
        .logarithmic(true)
        .clamping(SliderClamping::Never);
    let brighter = slid(ui, brightness, "Brightness", BRIGHTNESS_SAYS);
    let sharper = slid(ui, contrast, "Contrast", CONTRAST_SAYS);
    (brighter || sharper).then(|| WindowLevel::new(level, width.max(1.0)))
}

/// Draw the playback speed as a slider; whether it was moved.
pub(super) fn speed(ui: &mut Ui, fps: &mut f32) -> bool {
    let slider = Slider::new(fps, Cine::MIN_FPS..=Cine::MAX_FPS)
        .text("Speed")
        .show_value(false);
    slid(ui, slider, "Speed", "Slices shown each second")
}

/// Draw a slider through the slices of the active viewport, counted from 1 as the corner of
/// the picture counts them; the slice it was moved to, counting from 0, if it was. Nothing is
/// drawn for a series of one slice or none.
pub(super) fn slice(ui: &mut Ui, state: &AppState) -> Option<usize> {
    let count = state.slices_in(state.active);
    let at = state.viewports.get(state.active)?.slice;
    if count < 2 {
        return None;
    }
    let mut shown = at + 1;
    let slider = Slider::new(&mut shown, 1..=count)
        .text("Slice")
        .integer()
        .show_value(false);
    let moved = slid(ui, slider, "Slice", "Drag to go through the slices");
    (moved && shown != at + 1).then(|| shown.saturating_sub(1))
}

/// Add a slider, called `name` for a screen reader and saying `hint` when the pointer rests on
/// it; whether it was moved.
fn slid(ui: &mut Ui, slider: Slider<'_>, name: &str, hint: &str) -> bool {
    let response = ui.add(slider).on_hover_text(hint);
    access::call(&response, name);
    response.changed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sliders_reach_half_as_far_again_as_the_slice_spans_on_either_side() {
        let around = WindowLevel::new(300.0, 400.0);
        assert_eq!(reach(around, around), (-100.0, 700.0, 800.0));
    }

    #[test]
    fn a_window_typed_beyond_the_reach_is_held_whole() {
        let around = WindowLevel::new(300.0, 400.0);
        let shown = WindowLevel::new(2000.0, 5000.0);
        assert_eq!(reach(around, shown), (-100.0, 2000.0, 5000.0));
        let below = WindowLevel::new(-900.0, 10.0);
        assert_eq!(reach(around, below).0, -900.0);
    }

    #[test]
    fn a_window_of_no_width_still_gives_the_sliders_room() {
        let flat = WindowLevel::new(5.0, 0.0);
        let (low, high, widest) = reach(flat, flat);
        assert!(low < 5.0 && high > 5.0 && widest >= 2.0);
    }
}
