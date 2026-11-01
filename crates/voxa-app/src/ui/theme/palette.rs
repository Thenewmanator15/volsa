//! The colours of the panels and the bars, one set for each theme, and the contrast between
//! two colours.
//!
//! The pictures are not here: their ground, the colours of the planes and what is drawn over
//! a scan are the same in every theme (see the constants in [`super`]), so that a light
//! window never lowers the contrast of a letter or an outline over a scan.

use egui::{Color32, Context, Theme};

/// The colours of what is not a picture, in one theme: every piece of text in it meets the
/// contrast level AA asks (4.5 to 1) against both of the grounds, which a test holds it to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Panels, bars and windows: the ground text is drawn on.
    pub panel: Color32,
    /// A card or a button under the pointer, and the row chosen in the search.
    pub raised: Color32,
    /// Text.
    pub text: Color32,
    /// Text that matters less.
    pub quiet_text: Color32,
    /// The one accent: selections, links, and the outline of what is shown.
    pub accent: Color32,
    /// Marks something the person should look at.
    pub warning: Color32,
}

impl Palette {
    /// Dark panels, as Voxa has always had them.
    pub const DARK: Self = Self {
        panel: Color32::from_rgb(24, 27, 33),
        raised: Color32::from_rgb(38, 43, 52),
        text: Color32::from_rgb(214, 219, 227),
        quiet_text: Color32::from_rgb(150, 158, 170),
        accent: Color32::from_rgb(64, 156, 255),
        warning: Color32::from_rgb(240, 180, 60),
    };

    /// Light panels. The accent and the warning are darker than in the dark theme, as they
    /// must be to read on a light ground.
    pub const LIGHT: Self = Self {
        panel: Color32::from_rgb(246, 247, 249),
        raised: Color32::from_rgb(228, 232, 238),
        text: Color32::from_rgb(22, 26, 32),
        quiet_text: Color32::from_rgb(82, 90, 102),
        accent: Color32::from_rgb(0, 92, 184),
        warning: Color32::from_rgb(146, 82, 0),
    };

    /// The palette of this theme.
    pub const fn of(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::DARK,
            Theme::Light => Self::LIGHT,
        }
    }
}

/// The palette of the theme the context is drawing in now: the system's, or the one chosen.
pub fn palette(ctx: &Context) -> Palette {
    Palette::of(ctx.theme())
}

/// How bright a colour is, by the WCAG definition: 0 for black and 1 for white, from the
/// linear light of its channels. Its transparency is not considered.
#[cfg(test)]
fn luminance(colour: Color32) -> f32 {
    let linear = |channel: u8| {
        let level = f32::from(channel) / 255.0;
        if level <= 0.04045 {
            level / 12.92
        } else {
            ((level + 0.055) / 1.055).powf(2.4)
        }
    };
    let [red, green, blue, _] = colour.to_array();
    0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
}

/// The contrast between two colours as the WCAG defines it: from 1, for the same colour, to 21,
/// for black and white; the lighter's luminance and a little over the darker's. Text meets
/// level AA at 4.5 and above. Either way round.
#[cfg(test)]
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
