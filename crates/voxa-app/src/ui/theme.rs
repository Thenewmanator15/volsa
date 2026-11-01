//! How Voxa looks, defined once.
//!
//! There are two looks, light and dark. The panels, the bars and their text take the colours of
//! the look in use (see [`palette`]), and so do the windows. The pictures do not: what is
//! behind them, the colours of the planes, and the letters, scales and outlines drawn over a
//! scan are constants here, the same in both, so that a light window never lowers the contrast
//! of a letter over a scan.

use egui::style::Selection;
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, ThemePreference, Visuals,
    vec2,
};

mod palette;

#[cfg(test)]
pub use palette::contrast;
pub use palette::{Palette, palette};

/// The accent as it is drawn over the pictures, in both themes: the outline of the active
/// viewport, the handles, and annotations that are made. The panels' accent is
/// [`Palette::accent`], which is this colour in the dark theme and a darker one in the light.
pub const ACCENT: Color32 = Color32::from_rgb(64, 156, 255);
/// Corner radius of buttons, cards and windows, in points.
pub const ROUNDING: u8 = 8;
/// Behind the pictures, in both themes.
pub const BACKGROUND: Color32 = Color32::from_rgb(14, 16, 20);
/// Marks something the person should look at, as it is drawn over the pictures, in both themes.
/// The panels' is [`Palette::warning`].
pub const WARNING: Color32 = Color32::from_rgb(240, 180, 60);
/// The axial plane, in the colour 3D Slicer gives it, which most people who have seen plane
/// views will know: red.
pub const AXIAL: Color32 = Color32::from_rgb(235, 80, 80);
/// The sagittal plane: yellow.
pub const SAGITTAL: Color32 = Color32::from_rgb(235, 205, 70);
/// The coronal plane: green.
pub const CORONAL: Color32 = Color32::from_rgb(110, 190, 90);
/// Text that matters less, as it is drawn over the pictures, in both themes. The panels' is
/// [`Palette::quiet_text`].
pub const QUIET_TEXT: Color32 = Color32::from_rgb(150, 158, 170);

/// What is behind a selection in the dark theme: the light theme's accent, a blue that white
/// reads on at 6.5 to 1. The dark theme's own accent is too light for white text, and dark text
/// there would leave a focused field no outline that shows.
const DARK_SELECTION: Color32 = Palette::LIGHT.accent;
/// What is behind a selection in the light theme: a tint of the accent that the accent and the
/// text both read on, since the text of a selection is drawn over it.
const LIGHT_SELECTION: Color32 = Color32::from_rgb(190, 220, 255);

/// Shown word for word in the main window and in the About window.
pub const DISCLAIMER: &str = "Voxa is not a medical device and is not for diagnostic use.";
/// A window shorter than this, in points of the interface, has the lines of its bars set
/// closer together, to leave more of its height to the pictures.
pub const SHORT: f32 = 720.0;

/// Which theme the window is drawn in: the system's, or light or dark whatever the system
/// says. Chosen from the top bar's button and remembered.
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

    /// The word the choice is kept under between runs.
    pub const fn name(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// The choice kept under this word; `None` for a word that is not one of the three.
    pub fn named(word: &str) -> Option<Self> {
        [Self::System, Self::Light, Self::Dark]
            .into_iter()
            .find(|choice| choice.name() == word)
    }

    /// How egui is told to choose between its two looks.
    const fn preference(self) -> ThemePreference {
        match self {
            Self::System => ThemePreference::System,
            Self::Light => ThemePreference::Light,
            Self::Dark => ThemePreference::Dark,
        }
    }
}

/// The look of one theme: its palette on the panels and the windows, the accent on selections
/// and links, rounded corners, and text that meets the palette's contrast on both its grounds.
fn visuals(theme: Theme) -> Visuals {
    let palette = Palette::of(theme);
    let rounded = CornerRadius::same(ROUNDING);
    let (mut visuals, behind_fields, selection) = match theme {
        Theme::Dark => {
            // The stroke is the text of what is selected and the outline of a field that has
            // the keyboard, so it must read on the selection and on a field: white does both.
            let selection = Selection {
                bg_fill: DARK_SELECTION,
                stroke: Stroke::new(1.0, Color32::WHITE),
            };
            (Visuals::dark(), BACKGROUND, selection)
        }
        Theme::Light => {
            let selection = Selection {
                bg_fill: LIGHT_SELECTION,
                stroke: Stroke::new(1.0, palette.accent),
            };
            (Visuals::light(), Color32::WHITE, selection)
        }
    };
    visuals.selection = selection;
    visuals.hyperlink_color = palette.accent;
    visuals.panel_fill = palette.panel;
    visuals.window_fill = palette.panel;
    visuals.extreme_bg_color = behind_fields;
    visuals.weak_text_color = Some(palette.quiet_text);
    visuals.window_corner_radius = rounded;
    visuals.menu_corner_radius = rounded;
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.fg_stroke.color = palette.text;
    widgets.noninteractive.bg_fill = palette.panel;
    widgets.noninteractive.weak_bg_fill = palette.panel;
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
    visuals
}

/// Draw a context in the theme `choice`: the system's, or light or dark whatever the system
/// says. Both looks are set in it, with one accent colour, rounded corners and Voxa's own
/// sizes of type; `choice` says which is drawn.
pub fn apply(ctx: &egui::Context, choice: ThemeChoice) {
    ctx.set_theme(choice.preference());
    ctx.set_visuals_of(Theme::Dark, visuals(Theme::Dark));
    ctx.set_visuals_of(Theme::Light, visuals(Theme::Light));
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

/// Set the lines of a bar closer together in a window shorter than [`SHORT`]: the buttons
/// a little less tall and the lines a little nearer.
pub fn closer_in_a_short_window(ui: &mut egui::Ui) {
    if ui.ctx().content_rect().height() < SHORT {
        let spacing = &mut ui.style_mut().spacing;
        spacing.item_spacing = vec2(6.0, 4.0);
        spacing.button_padding = vec2(8.0, 2.0);
    }
}

/// Draw what `ui` holds from now on in the dark look, whichever theme the window is in: for what
/// lies on the pictures' ground, which is dark in both, such as the strip above the pictures.
pub fn on_the_pictures_ground(ui: &mut egui::Ui) {
    let dark = ui.ctx().style_of(Theme::Dark);
    ui.set_style(dark);
}

#[cfg(test)]
mod tests;
