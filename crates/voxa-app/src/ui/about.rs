//! The About window: what Voxa is and what it is for, its version, its licences and credits
//! and the disclaimer.
//! And the window it opens, that shows the notices of everything Voxa is built from.

use std::sync::LazyLock;

use egui::RichText;

use super::theme;

/// What is said about licences and credits, a line each.
pub const LICENCES: [&str; 6] = [
    "Voxa is licensed under either the MIT licence or the Apache License 2.0, as you prefer.",
    "It is built on other people's libraries, each under its own open-source licence.",
    "Typefaces: Ubuntu Light (Ubuntu Font Licence 1.0), Hack (MIT, and the Bitstream Vera \
     licence for its Bitstream Vera glyphs), Noto Emoji (SIL Open Font Licence 1.1) and \
     emoji-icon-font (MIT).",
    "Labelling is a port of SynthSeg's code (Copyright 2020 Benjamin Billot, Apache License \
     2.0), changed as NOTICE describes.",
    "This software is based in part on the work of the Independent JPEG Group.",
    "THIRD-PARTY-NOTICES.txt and NOTICE, beside the program, hold the full texts.",
];

/// What Voxa is for, as `docs/intended-use.md` says it: the window shows this and then the
/// disclaimer, which together are that page's first paragraph, word for word.
pub const INTENDED_USE: &str = "Voxa is for looking at MRI scans on your own computer: your \
     own scans, and scans used for learning, teaching or research. It shows the images, lets you \
     measure them and can show labels made by a model, but it does not diagnose, screen for or \
     rule out anything, and its measurements and labels are not clinical results. Questions \
     about anyone's health belong with a qualified clinician.";

/// What is said above the path of the log.
pub const LOG_SAYS: &str =
    "A log of what Voxa did is kept here. It holds nothing about a patient and no file names.";

/// Voxa's version, as written in its package.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Voxa's own notice, with the credit to `SynthSeg`: the `NOTICE` file that comes with it.
pub const NOTICE: &str = include_str!("../../../../NOTICE");
/// The licence and notice texts of every package built into Voxa: the
/// `THIRD-PARTY-NOTICES.txt` that comes with it, which `cargo xtask notices` writes. Kept in
/// the program too, so that they go wherever it is copied.
pub const THIRD_PARTY: &str = include_str!("../../../../THIRD-PARTY-NOTICES.txt");

/// What the notices window shows, a line each: [`NOTICE`], then [`THIRD_PARTY`].
static LINES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    NOTICE
        .lines()
        .chain([""])
        .chain(THIRD_PARTY.lines())
        .collect()
});

/// The interface's own note, kept between frames, that the notices window is open.
fn notices_open() -> egui::Id {
    egui::Id::new("voxa: the notices window is open")
}

/// Open the notices window: what the button in the About window does.
fn open_notices(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(notices_open(), true));
}

/// Show the About window while `open` is true; closing it sets `open` to false. The notices
/// window it opens is shown here too, for as long as it is open.
pub fn show(ctx: &egui::Context, open: &mut bool) {
    // As high as what it says needs, up to the height of Voxa's own window.
    egui::Window::new("About Voxa")
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_height(ctx.content_rect().height())
        .show(ctx, |ui| {
            // The window is held within Voxa's own, which would cut off what does not fit
            // below; what does not fit scrolls instead.
            let bottom = ui.ctx().content_rect().bottom() - ui.spacing().window_margin.bottomf();
            let room = (bottom - ui.cursor().top())
                .max(ui.text_style_height(&egui::TextStyle::Body) * 4.0);
            let area = egui::ScrollArea::vertical().max_height(room);
            area.show(ui, about);
        });
    let id = notices_open();
    let mut shown = ctx.data(|data| data.get_temp::<bool>(id)).unwrap_or(false);
    if shown {
        notices(ctx, &mut shown);
        ctx.data_mut(|data| data.insert_temp(id, shown));
    }
}

/// What the About window says, from the top.
fn about(ui: &mut egui::Ui) {
    ui.heading("Voxa");
    ui.label(format!("Version {VERSION}"));
    ui.label("A viewer for MRI scans.");
    ui.add_space(8.0);
    ui.label(INTENDED_USE);
    ui.label(
        RichText::new(theme::DISCLAIMER)
            .strong()
            .color(theme::WARNING),
    );
    ui.add_space(8.0);
    for line in LICENCES {
        ui.label(RichText::new(line).small().color(theme::QUIET_TEXT));
    }
    if ui.small_button("Licences and notices…").clicked() {
        open_notices(ui.ctx());
    }
    if let Some(log) = crate::log::path() {
        ui.add_space(8.0);
        ui.label(RichText::new(LOG_SAYS).small().color(theme::QUIET_TEXT));
        ui.horizontal(|ui| {
            let path = log.display().to_string();
            ui.label(RichText::new(&path).small().monospace());
            if ui.small_button("Copy").clicked() {
                ui.ctx().copy_text(path);
            }
        });
    }
}
/// Show every line of the notices while `open` is true, in a window that scrolls both ways.
/// Only the lines in view are laid out, so the window stays quick however long the notices.
fn notices(ctx: &egui::Context, open: &mut bool) {
    // Wide enough for the 78 columns most of the texts are written to.
    egui::Window::new("Licences and notices")
        .open(open)
        .collapsible(false)
        .default_size([700.0, 440.0])
        .show(ctx, |ui| {
            let height = ui.text_style_height(&egui::TextStyle::Monospace);
            let area = egui::ScrollArea::both().auto_shrink(false);
            area.show_rows(ui, height, LINES.len(), |ui, rows| {
                for line in LINES.get(rows).unwrap_or_default() {
                    ui.add(egui::Label::new(RichText::new(*line).monospace()).extend());
                }
            });
        });
}

#[cfg(test)]
mod said_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_shows_and_can_be_left_open() {
        let ctx = egui::Context::default();
        let mut open = true;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| show(ui.ctx(), &mut open));
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
        assert!(open);
    }

    #[test]
    fn the_version_is_the_package_version() {
        assert!(
            !VERSION.is_empty() && VERSION.split('.').count() >= 3,
            "{VERSION}"
        );
        assert!(
            LICENCES
                .iter()
                .any(|line| line.contains("Independent JPEG Group"))
        );
    }
}
