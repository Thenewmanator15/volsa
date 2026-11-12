//! The themes: that the colours of each are readable, that the pictures stay dark in both,
//! and how a choice is drawn and followed.

use egui::{CentralPanel, Color32, Context, RawInput, Theme, vec2};

use super::palette::{Palette, contrast, palette};
use super::*;
use crate::ui::seen;

/// The least contrast, as the WCAG ratio of the lighter colour's luminance to the darker's,
/// that text may have against its background to meet level AA. Written out here, not taken
/// from the code under test.
const AA: f32 = 4.5;

/// The palette a pass of a window drawn as `choice` has, when the system says `system`.
fn palette_of(choice: ThemeChoice, system: Option<Theme>) -> Palette {
    let ctx = Context::default();
    apply(&ctx, choice);
    let input = RawInput {
        system_theme: system,
        ..RawInput::default()
    };
    let mut drawn = None;
    let mut output = ctx.run_ui(input, |ui| drawn = Some(palette(ui.ctx())));
    // No renderer is behind this pass to take the font texture.
    output.textures_delta.clear();
    drawn.expect("the pass ran")
}

#[test]
fn the_theme_is_dark_with_the_accent_on_selections_and_rounded_corners() {
    let ctx = egui::Context::default();
    apply(&ctx, ThemeChoice::Dark);
    let style = ctx.global_style();
    assert!(style.visuals.dark_mode);
    // The accent is on selections as a deeper blue that white text reads on, and on links as is.
    assert_eq!(style.visuals.selection.bg_fill, Palette::LIGHT.accent);
    assert_eq!(style.visuals.selection.stroke.color, Color32::WHITE);
    assert_eq!(style.visuals.hyperlink_color, ACCENT);
    let rounded = egui::CornerRadius::same(ROUNDING);
    assert_eq!(style.visuals.widgets.inactive.corner_radius, rounded);
    assert_eq!(style.visuals.window_corner_radius, rounded);
    assert_eq!(style.visuals.panel_fill, Palette::DARK.panel);
}

#[test]
fn the_light_theme_has_light_panels_the_accent_on_its_selections_and_the_same_corners() {
    let ctx = egui::Context::default();
    apply(&ctx, ThemeChoice::Light);
    let style = ctx.global_style();
    let light = Palette::LIGHT;
    assert!(!style.visuals.dark_mode);
    assert_eq!(style.visuals.panel_fill, light.panel);
    assert_eq!(style.visuals.window_fill, light.panel);
    assert_eq!(style.visuals.selection.stroke.color, light.accent);
    assert_eq!(style.visuals.hyperlink_color, light.accent);
    let rounded = egui::CornerRadius::same(ROUNDING);
    assert_eq!(style.visuals.widgets.inactive.corner_radius, rounded);
    assert_eq!(style.visuals.window_corner_radius, rounded);
    // The words nothing says a colour for, and the faint ones, are the palette's.
    assert_eq!(style.visuals.text_color(), light.text);
    assert_eq!(style.visuals.weak_text_color(), light.quiet_text);
    // And so in the dark.
    apply(&ctx, ThemeChoice::Dark);
    let style = ctx.style_of(Theme::Dark);
    assert_eq!(style.visuals.text_color(), Palette::DARK.text);
    assert_eq!(style.visuals.weak_text_color(), Palette::DARK.quiet_text);
}

#[test]
fn each_theme_meets_aa_contrast() {
    for (theme, palette) in [("dark", Palette::DARK), ("light", Palette::LIGHT)] {
        for (ground, ground_name) in [(palette.panel, "panel"), (palette.raised, "raised")] {
            let text = [
                (palette.text, "text"),
                (palette.quiet_text, "quiet text"),
                (palette.accent, "accent"),
                (palette.warning, "warning"),
            ];
            for (colour, name) in text {
                let ratio = contrast(colour, ground);
                assert!(
                    ratio >= AA,
                    "{theme}: {name} {colour:?} on {ground_name} {ground:?} is {ratio:.2}:1"
                );
            }
        }
    }
}

#[test]
fn the_themes_are_not_the_same_and_the_text_is_louder_than_the_quiet_text() {
    assert_ne!(Palette::DARK, Palette::LIGHT);
    assert!(Palette::DARK.panel.r() < 128, "dark panels are dark");
    assert!(Palette::LIGHT.panel.r() > 128, "light panels are light");
    for palette in [Palette::DARK, Palette::LIGHT] {
        let louder =
            contrast(palette.text, palette.panel) > contrast(palette.quiet_text, palette.panel);
        assert!(louder, "{palette:?}");
    }
}

#[test]
fn contrast_is_the_ratio_of_luminances_the_wcag_way() {
    let ratio = contrast(Color32::BLACK, Color32::WHITE);
    assert!((ratio - 21.0).abs() < 0.01, "{ratio}");
    assert_eq!(
        contrast(Color32::WHITE, Color32::BLACK),
        ratio,
        "either way"
    );
    let grey = Color32::from_gray(119);
    assert!((contrast(grey, grey) - 1.0).abs() < 1e-6);
    // Mid grey on white, the figure the WCAG documents give: 4.48:1.
    let ratio = contrast(Color32::from_gray(119), Color32::WHITE);
    assert!((ratio - 4.48).abs() < 0.02, "{ratio}");
}

#[test]
fn the_pictures_stay_dark_in_the_light_theme() {
    assert_eq!(BACKGROUND, Color32::from_rgb(14, 16, 20), "today's colour");
    // The same in both themes: it is a constant, not a part of a palette.
    for choice in [ThemeChoice::Dark, ThemeChoice::Light] {
        let ctx = seen::context_in(1.0, choice);
        let ((), drawn) = seen::pass(&ctx, [900.0, 600.0], Vec::new(), |ui| {
            CentralPanel::default()
                .frame(crate::ui::middle())
                .show(ui, |ui| {
                    ui.allocate_space(ui.available_size());
                });
        });
        assert!(drawn.colours.contains(&BACKGROUND), "{choice:?}");
        assert!(
            !drawn.colours.contains(&palette(&ctx).panel),
            "the pictures' ground is not a panel's: {choice:?}"
        );
    }
    // And what is drawn over them keeps its colours: none of them is a palette's.
    for palette in [Palette::DARK, Palette::LIGHT] {
        let constants = [AXIAL, SAGITTAL, CORONAL, BACKGROUND];
        assert!(
            constants
                .iter()
                .all(|c| ![palette.panel, palette.raised].contains(c))
        );
    }
}

#[test]
fn system_follows_the_operating_system_and_light_and_dark_do_not() {
    let (light, dark) = (Some(Theme::Light), Some(Theme::Dark));
    assert_eq!(palette_of(ThemeChoice::System, light), Palette::LIGHT);
    assert_eq!(palette_of(ThemeChoice::System, dark), Palette::DARK);
    // When the system does not say, dark, as Volsa has always been.
    assert_eq!(palette_of(ThemeChoice::System, None), Palette::DARK);
    for system in [light, dark, None] {
        assert_eq!(palette_of(ThemeChoice::Light, system), Palette::LIGHT);
        assert_eq!(palette_of(ThemeChoice::Dark, system), Palette::DARK);
    }
}

#[test]
fn the_window_changes_theme_when_the_choice_does() {
    let ctx = Context::default();
    let size = vec2(300.0, 200.0);
    let panel = |ctx: &Context| {
        let input = RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..RawInput::default()
        };
        let mut fill = Color32::TRANSPARENT;
        let mut output = ctx.run_ui(input, |ui| fill = ui.visuals().panel_fill);
        output.textures_delta.clear();
        fill
    };
    apply(&ctx, ThemeChoice::Light);
    assert_eq!(panel(&ctx), Palette::LIGHT.panel);
    apply(&ctx, ThemeChoice::Dark);
    assert_eq!(panel(&ctx), Palette::DARK.panel);
    apply(&ctx, ThemeChoice::Light);
    assert_eq!(panel(&ctx), Palette::LIGHT.panel, "and back again");
}

#[test]
fn a_choice_is_kept_under_a_name_and_read_back_from_it() {
    for choice in [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark] {
        assert_eq!(ThemeChoice::named(choice.name()), Some(choice));
        assert!(crate::prefs::THEMES.contains(&choice.name()), "{choice:?}");
    }
    assert_eq!(ThemeChoice::named("sepia"), None);
    assert_eq!(ThemeChoice::named(""), None);
    assert_eq!(ThemeChoice::named("Dark"), None, "the names are lower case");
    assert_eq!(crate::prefs::THEMES.len(), 3);
    assert_eq!(ThemeChoice::default().name(), "system");
}

#[test]
fn a_short_window_has_its_bars_set_closer() {
    let ctx = egui::Context::default();
    apply(&ctx, ThemeChoice::Dark);
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
        "Volsa is not a medical device and is not for diagnostic use."
    );
}

#[test]
fn text_in_fields_on_buttons_and_in_selections_reads_in_each_theme() {
    for (name, theme) in [("dark", Theme::Dark), ("light", Theme::Light)] {
        let (palette, visuals) = (Palette::of(theme), visuals(theme));
        // Typed into a field.
        for ink in [palette.text, palette.quiet_text] {
            let field = visuals.extreme_bg_color;
            assert!(contrast(ink, field) >= AA, "{name}: {ink:?} in a field");
        }
        // On a button, a bar and what is under the pointer, in each state of them.
        let widgets = &visuals.widgets;
        let states = [
            ("at rest, not to be pressed", widgets.noninteractive),
            ("at rest", widgets.inactive),
            ("under the pointer", widgets.hovered),
            ("pressed", widgets.active),
            ("open", widgets.open),
        ];
        for (state, look) in states {
            for fill in [look.weak_bg_fill, look.bg_fill] {
                let ratio = contrast(look.fg_stroke.color, fill);
                assert!(
                    ratio >= AA,
                    "{name}: text {state} is {ratio:.2}:1 on its fill"
                );
            }
        }
        // And a selection: a selected tab, and selected text.
        let selection = visuals.selection;
        let ratio = contrast(selection.stroke.color, selection.bg_fill);
        assert!(ratio >= AA, "{name}: selected text is {ratio:.2}:1");
    }
}

#[test]
fn the_strip_over_the_pictures_is_drawn_dark_and_reads_on_their_ground_in_both_themes() {
    let state = crate::testing::thick_scan_ready();
    for choice in [ThemeChoice::Dark, ThemeChoice::Light] {
        let ctx = seen::context_in(1.0, choice);
        let ((), drawn) = seen::pass(&ctx, [900.0, 600.0], Vec::new(), |ui| {
            CentralPanel::default()
                .frame(crate::ui::middle())
                .show(ui, |ui| {
                    on_the_pictures_ground(ui);
                    let along = ui.available_rect_before_wrap();
                    crate::ui::strip::show(ui, along, &state, &mut Vec::new());
                });
        });
        let said = drawn.words.iter().any(|(word, _)| word == "Planes");
        assert!(said, "{choice:?}: the strip is drawn: {:?}", drawn.words);
        // The two chosen are on the accent, which the selection tests hold to account.
        let faint = drawn.hard_to_read_on(BACKGROUND, &["Slices", "1"]);
        assert!(
            faint.is_empty(),
            "{choice:?} on the pictures' ground: {faint:?}"
        );
    }
}

#[test]
fn a_focused_field_keeps_an_outline_that_shows_in_each_theme() {
    // egui outlines the field that has the keyboard in the selection's stroke, and an unfocused
    // one in nothing. The outline must show against the field and against what it sits on.
    let at_least = 3.0;
    for (name, theme) in [("dark", Theme::Dark), ("light", Theme::Light)] {
        let (palette, visuals) = (Palette::of(theme), visuals(theme));
        let outline = visuals.selection.stroke.color;
        let field = visuals.text_edit_bg_color();
        let on_the_field = contrast(outline, field);
        assert!(
            on_the_field >= at_least,
            "{name}: {on_the_field:.2}:1 on the field"
        );
        let on_the_panel = contrast(outline, palette.panel);
        assert!(
            on_the_panel >= at_least,
            "{name}: {on_the_panel:.2}:1 on the panel"
        );
        assert_ne!(outline, field, "{name}: it is not the field's own colour");
        assert_eq!(
            visuals.widgets.inactive.bg_stroke.width, 0.0,
            "{name}: none unfocused"
        );
    }
}
