//! The list of keys, opened with `?`.

use egui::RichText;

use super::overlay::SHORTCUTS;
use super::theme;

/// What the mouse does over a viewport, for the same window.
pub const MOUSE: [(&str, &str); 9] = [
    ("Wheel", "Move through the slices"),
    ("Ctrl + wheel", "Zoom about the pointer"),
    ("Middle drag", "Move the picture"),
    (
        "Right drag",
        "Change the window: across for width, down for centre",
    ),
    ("Double-click", "Fit the whole picture in the viewport"),
    ("Drag, in 3D", "Turn the scene"),
    ("Shift + drag, or middle drag, in 3D", "Move the scene"),
    ("Wheel, in 3D", "Zoom about the pointer"),
    ("Double-click, in 3D", "Put that point in the middle"),
];

/// Show the reference while `open` is true; closing it sets `open` to false.
pub fn show(ctx: &egui::Context, open: &mut bool) {
    egui::Window::new("Keys and mouse")
        .open(open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("shortcuts")
                .num_columns(2)
                .spacing([24.0, 6.0])
                .show(ui, |ui| {
                    for (input, does) in SHORTCUTS.iter().chain(&MOUSE) {
                        ui.label(RichText::new(*input).monospace().color(theme::ACCENT));
                        ui.label(*does);
                        ui.end_row();
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_draws_and_lists_every_key_and_mouse_action() {
        let ctx = egui::Context::default();
        let mut open = true;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| show(ui.ctx(), &mut open));
        // No renderer is behind this pass to take the font texture.
        output.textures_delta.clear();
        assert!(open);
        assert_eq!(SHORTCUTS.len() + MOUSE.len(), 19);
        let in_3d = MOUSE
            .iter()
            .filter(|(input, _)| input.ends_with("in 3D"))
            .count();
        assert_eq!(in_3d, 4, "what the mouse does in the 3D view is listed too");
    }
}
