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

/// The height of the reference's title bar and margins, which its list leaves room for.
const ABOUT_THE_LIST: f32 = 72.0;

/// Show the reference while `open` is true; closing it sets `open` to false. The whole list
/// is shown when the Voxa window is tall enough, and scrolls when it is not.
pub fn show(ctx: &egui::Context, open: &mut bool) {
    let room = (ctx.content_rect().height() - ABOUT_THE_LIST).max(ABOUT_THE_LIST);
    egui::Window::new("Keys and mouse")
        .open(open)
        .collapsible(false)
        .auto_sized()
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(room)
                .show(ui, |ui| {
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
        assert_eq!(SHORTCUTS.len() + MOUSE.len(), 31);
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

    /// Where the reference is drawn in a Voxa window of this size, in points.
    fn drawn_in(width: f32, height: f32) -> egui::Rect {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        let mut open = true;
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..egui::RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| show(ui.ctx(), &mut open));
            output.textures_delta.clear();
        }
        // A window is known by its title, as egui keeps it.
        let id = egui::Id::new(Some("Keys and mouse"));
        let drawn = ctx.memory(|memory| memory.area_rect(id));
        drawn.expect("the window was drawn")
    }

    #[test]
    fn the_reference_stays_inside_the_smallest_window_and_is_whole_in_the_usual_one() {
        let smallest = drawn_in(900.0, 600.0);
        assert!(
            smallest.bottom() <= 600.0,
            "it runs off the bottom: {smallest:?}"
        );
        let usual = drawn_in(1440.0, 900.0);
        assert!(usual.bottom() <= 900.0, "it runs off the bottom: {usual:?}");
        // Every one of its lines is at least 20 points high: the whole list is shown.
        let lines = (SHORTCUTS.len() + MOUSE.len()) as f32;
        assert!(usual.height() > lines * 20.0, "it is cut short: {usual:?}");
    }
}
