//! The list of keys, opened with `?`.

use egui::RichText;
use egui::scroll_area::ScrollAreaOutput;

use super::overlay::SHORTCUTS;
use super::scrolled;
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

/// What the window's frame and title take of the height of the window it is shown in.
const AROUND: f32 = 64.0;
/// How wide the reference is, at most: room for each key beside what it does.
const WIDEST: f32 = 860.0;
/// Narrower than this, what each key does follows it on its line rather than in a column.
const BESIDE: f32 = 760.0;

/// The interface's own name for the window, which it keeps its place under.
fn named() -> egui::Id {
    egui::Id::new("voxa: keys and mouse")
}

/// Show the reference while `open` is true; closing it sets `open` to false. In a window too
/// small to hold it all, it scrolls, with its bar in sight; in a narrow one, what each key
/// does follows it. Gives what egui said of its list, while it is shown.
pub fn show(ctx: &egui::Context, open: &mut bool) -> Option<ScrollAreaOutput<()>> {
    let room = ctx.content_rect().size();
    let wide = (room.x - AROUND).clamp(AROUND, WIDEST);
    let shown = egui::Window::new("Keys and mouse")
        .id(named())
        .open(open)
        .collapsible(false)
        .auto_sized()
        .show(ctx, |ui| {
            ui.set_width(wide);
            // The bar is always drawn, so that a list cut short does not look whole.
            scrolled::solid_bars(ui);
            let most = (room.y - 2.0 * AROUND).max(AROUND);
            scrolled::up_and_down(most).show(ui, |ui| {
                if wide >= BESIDE {
                    side_by_side(ui);
                } else {
                    run_on(ui);
                }
            })
        });
    shown.and_then(|window| window.inner)
}

/// A key or mouse action, as the reference names it.
fn key(input: &str) -> RichText {
    RichText::new(input).monospace().color(theme::ACCENT)
}

/// Each key with what it does beside it, in two columns.
fn side_by_side(ui: &mut egui::Ui) {
    egui::Grid::new("shortcuts")
        .num_columns(2)
        .spacing([24.0, 6.0])
        .show(ui, |ui| {
            for (input, does) in SHORTCUTS.iter().chain(&MOUSE) {
                ui.label(key(input));
                ui.add(egui::Label::new(*does).wrap());
                ui.end_row();
            }
        });
}

/// Each key followed by what it does, on as many lines as that takes, for a narrow window.
fn run_on(ui: &mut egui::Ui) {
    let keys = egui::TextStyle::Monospace.resolve(ui.style());
    let words = egui::TextStyle::Body.resolve(ui.style());
    let colour = ui.visuals().text_color();
    for (input, does) in SHORTCUTS.iter().chain(&MOUSE) {
        let mut line = egui::text::LayoutJob::default();
        let named = egui::TextFormat::simple(keys.clone(), theme::ACCENT);
        line.append(input, 0.0, named);
        line.append(does, 12.0, egui::TextFormat::simple(words.clone(), colour));
        ui.add(egui::Label::new(line).wrap());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::scrolled::{Scrolled, bar_drawn};

    #[test]
    fn the_reference_draws_and_lists_every_key_and_mouse_action() {
        let ctx = egui::Context::default();
        let mut open = true;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(ui.ctx(), &mut open);
        });
        // No renderer is behind this pass to take the font texture.
        output.textures_delta.clear();
        assert!(open);
        assert_eq!(SHORTCUTS.len() + MOUSE.len(), 34);
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

    /// The reference shown in a Voxa window `width` by `height` points large, in Voxa's
    /// look: what its list held and showed, whether the list's bar is drawn, and where the
    /// reference is.
    fn drawn_in(width: f32, height: f32) -> (Scrolled, bool, egui::Rect) {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        let mut open = true;
        let (mut scrolled, mut bar) = (None, false);
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..egui::RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                scrolled = show(ui.ctx(), &mut open).map(|list| Scrolled::of(&list));
            });
            // No renderer is behind this pass to take the font texture.
            output.textures_delta.clear();
            let shown = scrolled.map_or(egui::Rect::NOTHING, |scrolled| scrolled.shown);
            bar = bar_drawn(&output.shapes, shown);
        }
        let shown = ctx.memory(|memory| memory.area_rect(named()));
        let shown = shown.expect("the reference is shown");
        (scrolled.expect("its list is laid out"), bar, shown)
    }

    #[test]
    fn where_the_list_is_cut_short_its_bar_is_drawn() {
        // In windows of these sizes the list is longer than the room for it, as the window
        // itself shows: the bar beside it says so, with no pointer over it.
        for (width, height) in [(900.0, 600.0), (1440.0, 900.0)] {
            let (scrolled, bar, shown) = drawn_in(width, height);
            assert!(scrolled.is_cut(), "the list is whole at {width}x{height}");
            assert!(bar, "no bar beside the list at {width}x{height}: {shown:?}");
            assert!(
                shown.bottom() <= height,
                "it runs off the bottom: {shown:?}"
            );
        }
    }

    #[test]
    fn the_list_is_whole_in_a_large_window_with_its_bar_still_drawn() {
        let (scrolled, bar, shown) = drawn_in(2560.0, 1600.0);
        assert!(!scrolled.is_cut(), "{scrolled:?}");
        assert!(bar, "no bar beside the list: {shown:?}");
    }

    #[test]
    fn the_reference_fits_in_the_smallest_window() {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0));
        let (_, _, shown) = drawn_in(640.0, 480.0);
        assert!(screen.contains_rect(shown), "{shown:?}");
        assert!(
            shown.width() > 0.8 * screen.width(),
            "the words take the room there is, not a narrow strip of it: {shown:?}"
        );
    }
}
