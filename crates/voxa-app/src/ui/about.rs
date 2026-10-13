//! The About window: what Voxa is, its version, its licences and the disclaimer.

use egui::RichText;

use super::theme;

/// What is said about licences, a line each.
pub const LICENCES: [&str; 4] = [
    "Voxa is licensed under either the MIT licence or the Apache License 2.0, as you prefer.",
    "It is built on other people's libraries, each under its own permissive licence.",
    "Typefaces: Ubuntu (Ubuntu Font Licence 1.0) and Hack (MIT).",
    "This software is based in part on the work of the Independent JPEG Group.",
];

/// What is said above the path of the log.
pub const LOG_SAYS: &str =
    "A log of what Voxa did is kept here. It holds nothing about a patient and no file names.";

/// Voxa's version, as written in its package.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Show the About window while `open` is true; closing it sets `open` to false.
pub fn show(ctx: &egui::Context, open: &mut bool) {
    egui::Window::new("About Voxa")
        .open(open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Voxa");
            ui.label(format!("Version {VERSION}"));
            ui.label("A viewer for MRI scans.");
            ui.add_space(8.0);
            ui.label(
                RichText::new(theme::DISCLAIMER)
                    .strong()
                    .color(theme::WARNING),
            );
            ui.add_space(8.0);
            for line in LICENCES {
                ui.label(RichText::new(line).small().color(theme::QUIET_TEXT));
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
        });
}

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
