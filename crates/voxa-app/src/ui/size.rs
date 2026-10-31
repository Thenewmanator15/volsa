//! The size the interface is drawn at: the interface size menu in the top bar's menu, beside the
//! keys that egui itself answers to (Ctrl+= and Ctrl+- for larger and smaller, Ctrl+0 for the
//! usual size).

use egui::{RichText, Ui};

use super::commands::{self, Does, Request, menus};
use super::theme;
use crate::state::AppState;

/// The sizes the interface size menu offers: 1 is the interface's usual size.
pub const SIZES: [f32; 8] = [0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

/// An interface size as a percentage of the usual size, to the nearest whole one: `125%`.
pub fn percent(size: f32) -> String {
    format!("{:.0}%", size * 100.0)
}

/// The action that draws the interface at `size`, from the list of actions.
fn action_for(size: f32) -> Option<&'static commands::Command> {
    commands::all().iter().find(|command| {
        matches!(command.does, Does::Ask(Request::Size(at)) if percent(at) == percent(size))
    })
}

/// The interface size menu: the interface size now, and the sizes to choose between, each an
/// action of the list of actions. Gives the size chosen, if one was.
pub fn menu(ui: &mut Ui, state: &AppState) -> Option<f32> {
    let now = ui.ctx().zoom_factor();
    let mut chosen = None;
    let written = format!("Interface size {}", percent(now));
    commands::menu_button(ui, &menus::SIZE, &written, |ui| {
        for command in SIZES.into_iter().filter_map(action_for) {
            let this = command.label == percent(now);
            let pressed = commands::button(ui, command.id, state, Some(this));
            if let Some(Does::Ask(Request::Size(size))) = pressed {
                chosen = Some(size);
                ui.close();
            }
        }
        ui.separator();
        let keys = "Ctrl+= larger, Ctrl+- smaller, Ctrl+0 usual";
        ui.label(
            RichText::new(keys)
                .small()
                .color(theme::palette(ui.ctx()).quiet_text),
        );
    });
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
        assert!(menus::SIZE.explains.contains("Ctrl+0"));
    }

    /// One pass of the menu at the top left of a window, with these events.
    fn pass(ctx: &egui::Context, events: Vec<Event>) -> Option<f32> {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            events,
            ..RawInput::default()
        };
        let mut chosen = None;
        let mut output = ctx.run_ui(input, |ui| chosen = menu(ui, &AppState::default()));
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
