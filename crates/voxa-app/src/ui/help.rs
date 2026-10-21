//! The list of keys, opened with `?`.

use egui::RichText;

use super::overlay::SHORTCUTS;
use super::theme;

/// What the mouse does over a viewport, for the same window.
pub const MOUSE: [(&str, &str); 14] = [
    ("Wheel", "Move through the slices"),
    ("Ctrl + wheel", "Zoom about the pointer"),
    ("Middle drag", "Move the picture"),
    (
        "Right drag",
        "Change the window: across for width, down for centre",
    ),
    ("Double-click", "Fit the whole picture in the viewport"),
    (
        "Click or drag, in a plane view",
        "Move the position, or draw with the tool in hand",
    ),
    (
        "Click, in 3D",
        "Move the position to that surface, or measure with the tool in hand",
    ),
    ("Drag, in 3D", "Turn the scene"),
    ("Shift + drag, or middle drag, in 3D", "Move the scene"),
    ("Wheel, in 3D", "Zoom about the pointer"),
    ("Double-click, in 3D", "Put that point in the middle"),
    (
        "Click, on a labelled part",
        "Select it. Rest the pointer on a part for its name",
    ),
    (
        "Click Label this scan, in the Parts tab",
        "Label the scan with Voxa's own networks. The menu beside it chooses how",
    ),
    (
        "Click Stop, in the Parts tab",
        "Stop labelling the scan now. No labels are made",
    ),
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
        assert_eq!(SHORTCUTS.len() + MOUSE.len(), 28);
        let parts = MOUSE
            .iter()
            .find(|(input, _)| input.contains("labelled part"));
        let (_, does) = parts.expect("what a click on a part does is listed");
        assert!(does.contains("Select") && does.contains("name"), "{does}");
        let label = MOUSE
            .iter()
            .find(|(input, _)| input.contains("Label this scan"));
        let (_, does) = label.expect("what the button that labels does is listed");
        assert!(does.contains("Voxa's own networks"), "{does}");
        let stop = MOUSE.iter().find(|(input, _)| input.contains("Stop"));
        let (_, does) = stop.expect("what Stop does is listed");
        assert!(does.contains("Stop labelling"), "{does}");
        let in_3d = MOUSE
            .iter()
            .filter(|(input, _)| input.ends_with("in 3D"))
            .count();
        assert_eq!(in_3d, 5, "what the mouse does in the 3D view is listed too");
    }
}
