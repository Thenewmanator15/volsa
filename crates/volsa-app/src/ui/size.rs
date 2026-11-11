//! The size the interface is drawn at: the Size menu in the bar, beside the keys that egui
//! itself answers to (Ctrl+= and Ctrl+- for larger and smaller, Ctrl+0 for the usual size).

use egui::{RichText, Ui};

use super::theme;

/// The sizes the Size menu offers: 1 is the interface's usual size.
pub const SIZES: [f32; 8] = [0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

/// What the Size menu says when pointed at.
pub const SIZE_SAYS: &str = "How large the interface is drawn: its words, buttons and lists. \
                             Ctrl+= and Ctrl+- make it larger and smaller, and Ctrl+0 puts it \
                             back to its usual size.";

/// An interface size as a percentage of the usual size, to the nearest whole one: `125%`.
pub fn percent(size: f32) -> String {
    format!("{:.0}%", size * 100.0)
}

/// The Size menu: the interface size now, and the sizes to choose between. Gives the size
/// chosen, if one was.
pub fn menu(ui: &mut Ui) -> Option<f32> {
    let now = ui.ctx().zoom_factor();
    let mut chosen = None;
    let menu = ui.menu_button(format!("Size {}", percent(now)), |ui| {
        for size in SIZES {
            let this = percent(size) == percent(now);
            if ui.selectable_label(this, percent(size)).clicked() {
                chosen = Some(size);
                ui.close();
            }
        }
        ui.separator();
        let keys = "Ctrl+= larger, Ctrl+- smaller, Ctrl+0 usual";
        ui.label(RichText::new(keys).small().color(theme::QUIET_TEXT));
    });
    menu.response.on_hover_text(SIZE_SAYS);
    chosen
}

#[cfg(test)]
mod tests {
    use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};

    use super::*;
    use crate::prefs::INTERFACE_SIZES;

    #[test]
    fn the_interface_size_is_said_in_percent() {
        assert_eq!(percent(1.0), "100%");
        assert_eq!(percent(1.25), "125%");
        assert_eq!(percent(0.9), "90%");
        assert_eq!(percent(1.300_000_1), "130%");
    }

    #[test]
    fn the_sizes_offered_run_from_smaller_to_larger_and_each_can_be_remembered() {
        assert!(SIZES.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(SIZES.contains(&1.0), "the usual size");
        assert!(SIZES.iter().all(|size| INTERFACE_SIZES.contains(size)));
        assert!(SIZE_SAYS.contains("Ctrl+0"), "{SIZE_SAYS}");
    }

    /// One pass of the menu at the top left of a window, with these events.
    fn pass(ctx: &egui::Context, events: Vec<Event>) -> Option<f32> {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            events,
            ..RawInput::default()
        };
        let mut chosen = None;
        let mut output = ctx.run_ui(input, |ui| chosen = menu(ui));
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
        chosen
    }

    /// Press and release the main button at a point.
    fn click(ctx: &egui::Context, at: Pos2) -> Option<f32> {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let pressed = pass(ctx, vec![Event::PointerMoved(at), button(true)]);
        pressed.or(pass(ctx, vec![button(false)]))
    }

    /// Open the menu and click at `y` below its button: the size chosen, if any.
    fn open_and_click(y: f32) -> Option<f32> {
        let ctx = egui::Context::default();
        let _ = pass(&ctx, Vec::new());
        let opened = click(&ctx, pos2(20.0, 10.0));
        assert_eq!(opened, None, "opening it chooses nothing");
        // The menu is laid out unseen on its first frame, and can be used from the next.
        let _ = pass(&ctx, Vec::new());
        click(&ctx, pos2(30.0, y))
    }

    #[test]
    fn choosing_a_size_in_the_menu_gives_it_and_the_smallest_is_first() {
        // The menu opens below its button: the first size in it is the smallest.
        let chosen = (24..200u16)
            .step_by(4)
            .find_map(|y| open_and_click(f32::from(y)));
        assert_eq!(chosen, Some(SIZES[0]));
    }
}
