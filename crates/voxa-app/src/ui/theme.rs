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
/// A window shorter than this, in points of the interface, has the lines of its bars set
/// closer together, to leave more of its height to the pictures.
pub const SHORT: f32 = 720.0;

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

/// Which theme the window is drawn in: the system's, or light or dark whatever the system
/// says. Only dark is drawn so far: the top bar's button chooses, and a later change applies
/// the choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    /// As the system is set.
    #[default]
    System,
    /// Light.
    Light,
    /// Dark.
    Dark,
}

impl ThemeChoice {
    /// The choice the theme button moves to from this one: the system's, then light, then
    /// dark, and round again.
    pub const fn next(self) -> Self {
        match self {
            Self::System => Self::Light,
            Self::Light => Self::Dark,
            Self::Dark => Self::System,
        }
    }
}

/// Set the lines of a bar closer together in a window shorter than [`SHORT`]: the buttons
/// a little less tall and the lines a little nearer.
pub fn closer_in_a_short_window(ui: &mut egui::Ui) {
    if ui.ctx().content_rect().height() < SHORT {
        let spacing = &mut ui.style_mut().spacing;
        spacing.item_spacing = vec2(6.0, 4.0);
        spacing.button_padding = vec2(8.0, 2.0);
    }
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
    fn a_short_window_has_its_bars_set_closer() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let padding = |high: f32| {
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1280.0, high));
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..egui::RawInput::default()
            };
            let mut padding = egui::Vec2::ZERO;
            let mut output = ctx.run_ui(input, |ui| {
                closer_in_a_short_window(ui);
                padding = ui.spacing().button_padding;
            });
            // No renderer is behind this pass to take the font texture.
            output.textures_delta.clear();
            padding
        };
        assert_eq!(padding(900.0), vec2(10.0, 5.0));
        assert_eq!(padding(680.0), vec2(8.0, 2.0));
    }

    #[test]
    fn the_theme_button_goes_from_system_to_light_to_dark_and_round_again() {
        let mut choice = ThemeChoice::default();
        assert_eq!(choice, ThemeChoice::System);
        let seen: Vec<ThemeChoice> = (0..4)
            .map(|_| {
                choice = choice.next();
                choice
            })
            .collect();
        assert_eq!(
            seen,
            [
                ThemeChoice::Light,
                ThemeChoice::Dark,
                ThemeChoice::System,
                ThemeChoice::Light
            ]
        );
    }

    #[test]
    fn the_disclaimer_is_word_for_word() {
        assert_eq!(
            DISCLAIMER,
            "Voxa is not a medical device and is not for diagnostic use."
        );
    }
}
