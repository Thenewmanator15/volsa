//! How Voxa looks, defined once.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, vec2};

/// The one accent colour: selections, the active viewport, links.
pub const ACCENT: Color32 = Color32::from_rgb(64, 156, 255);
/// Corner radius of buttons, cards and windows, in points.
pub const ROUNDING: u8 = 8;
/// Behind everything.
pub const BACKGROUND: Color32 = Color32::from_rgb(14, 16, 20);
/// Panels and windows.
pub const PANEL: Color32 = Color32::from_rgb(24, 27, 33);
/// A card or button under the pointer.
pub const RAISED: Color32 = Color32::from_rgb(38, 43, 52);
/// Marks something the person should look at.
pub const WARNING: Color32 = Color32::from_rgb(240, 180, 60);
/// The axial plane, in the colour 3D Slicer gives it, which most people who have seen plane
/// views will know: red.
pub const AXIAL: Color32 = Color32::from_rgb(235, 80, 80);
/// The sagittal plane: yellow.
pub const SAGITTAL: Color32 = Color32::from_rgb(235, 205, 70);
/// The coronal plane: green.
pub const CORONAL: Color32 = Color32::from_rgb(110, 190, 90);
/// Text that matters less.
pub const QUIET_TEXT: Color32 = Color32::from_rgb(150, 158, 170);

/// Shown word for word in the main window and in the About window.
pub const DISCLAIMER: &str = "Voxa is not a medical device and is not for diagnostic use.";

/// Give a context Voxa's look: dark whatever the system says, one accent colour, rounded
/// corners, and its own sizes of type.
pub fn apply(ctx: &egui::Context) {
    let rounded = CornerRadius::same(ROUNDING);
    let mut visuals = egui::Visuals::dark();
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.hyperlink_color = ACCENT;
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = BACKGROUND;
    visuals.window_corner_radius = rounded;
    visuals.menu_corner_radius = rounded;
    let widgets = &mut visuals.widgets;
    let states = [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ];
    for state in states {
        state.corner_radius = rounded;
    }
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
    ctx.all_styles_mut(|style| {
        let sized = |size, family| FontId::new(size, family);
        style.text_styles = [
            (TextStyle::Heading, sized(18.0, FontFamily::Proportional)),
            (TextStyle::Body, sized(14.0, FontFamily::Proportional)),
            (TextStyle::Button, sized(14.0, FontFamily::Proportional)),
            (TextStyle::Small, sized(11.5, FontFamily::Proportional)),
            (TextStyle::Monospace, sized(13.0, FontFamily::Monospace)),
        ]
        .into();
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(10.0, 5.0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_is_dark_with_the_accent_on_selections_and_rounded_corners() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let style = ctx.global_style();
        assert!(style.visuals.dark_mode);
        assert_eq!(style.visuals.selection.bg_fill, ACCENT);
        let rounded = egui::CornerRadius::same(ROUNDING);
        assert_eq!(style.visuals.widgets.inactive.corner_radius, rounded);
        assert_eq!(style.visuals.window_corner_radius, rounded);
    }

    #[test]
    fn the_disclaimer_is_word_for_word() {
        assert_eq!(
            DISCLAIMER,
            "Voxa is not a medical device and is not for diagnostic use."
        );
    }
}
