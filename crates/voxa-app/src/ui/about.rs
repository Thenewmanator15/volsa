//! The About window: what Voxa is and what it is for, its version, its licences and credits
//! and the disclaimer.
//! And the window it opens, that shows the notices of everything Voxa is built from.

use std::sync::LazyLock;

use egui::RichText;
use egui::scroll_area::ScrollAreaOutput;

use super::{floating, scrolled, theme};
use crate::state::{Notice, NoticeKind};

/// What is said about licences and credits, a line each.
pub const LICENCES: [&str; 7] = [
    "Voxa is licensed under either the MIT licence or the Apache License 2.0, as you prefer.",
    "It is built on other people's libraries, each under its own open-source licence.",
    concat!(
        "The voxa.exe a release offers has Microsoft's C runtime built in, which is under ",
        "Microsoft's own terms, not an open-source licence."
    ),
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

/// What is said above the path of the log. A crash is logged by the place in the source code
/// where it happened, which for a library can be a path on the computer Voxa was built on.
pub const LOG_SAYS: &str = concat!(
    "A log of what Voxa did is kept here. It holds nothing about a patient and none of your ",
    "files' names. A crash is logged by the place in the source code of Voxa, or of a ",
    "library, where it happened."
);

/// The key of the notice that says where the log is: asking again takes the old one's place.
const LOG_NOTICE_KEY: &str = "log";
/// The title of the window that shows the notices of everything Voxa is built from.
const NOTICES_TITLE: &str = "Licences and notices";

/// Voxa's version, as written in its package.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The notice that says where the log of this run is, or that there is none.
pub fn where_the_log_is() -> Notice {
    log_notice(crate::log::path())
}

/// The notice that says the log is at `path`, if it is anywhere.
fn log_notice(path: Option<&std::path::Path>) -> Notice {
    let text = match path {
        Some(path) => format!("The log of this run is at {}.", path.display()),
        None => "Voxa is not keeping a log in this run.".to_owned(),
    };
    Notice {
        kind: NoticeKind::Info,
        text,
        details: Vec::new(),
        key: Some(LOG_NOTICE_KEY),
    }
}

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
pub(super) fn open_notices(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(notices_open(), true));
}

/// Close the notices window, which Esc does when it was opened last.
pub(super) fn close_notices(ctx: &egui::Context) {
    ctx.data_mut(|data| data.insert_temp(notices_open(), false));
}

/// Whether the notices window is open.
pub(super) fn notices_are_open(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(notices_open()))
        .unwrap_or(false)
}

/// The notices window's own name, which it keeps its place under.
fn notices_id() -> egui::Id {
    egui::Id::new("voxa: licences and notices")
}

/// What a window's frame and title take of the height of the window it is shown in.
const AROUND: f32 = 64.0;
/// The height kept below the text for the button that opens the notices.
const BUTTON_ROOM: f32 = 28.0;

/// The interface's own name for the About window, which it keeps its place under.
pub(super) fn id() -> egui::Id {
    egui::Id::new("voxa: about")
}

/// Show the About window while `open` is true; closing it sets `open` to false. In a window
/// too short to hold it all, its text scrolls, with the bar in sight, and the button that opens
/// the notices stays below it. The notices window is shown here too, for as long as it is
/// open. Gives what egui said of the text, while the window is shown.
pub fn show(ctx: &egui::Context, open: &mut bool) -> Option<ScrollAreaOutput<()>> {
    let room = floating::area(ctx).height();
    let window = egui::Window::new("About Voxa");
    let about = floating::over(window, ctx)
        .id(id())
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_height(room)
        .show(ctx, |ui| {
            scrolled::solid_bars(ui);
            let most = (room - 2.0 * AROUND - BUTTON_ROOM).max(AROUND);
            let text = scrolled::up_and_down(most).show(ui, contents);
            if ui.small_button("Licences and notices…").clicked() {
                open_notices(ui.ctx());
            }
            text
        });
    let id = notices_open();
    let mut shown = ctx.data(|data| data.get_temp::<bool>(id)).unwrap_or(false);
    if shown {
        notices(ctx, &mut shown);
        ctx.data_mut(|data| data.insert_temp(id, shown));
    }
    about.and_then(|window| window.inner)
}

/// What the About window says: what Voxa is, its version, the disclaimer, its licences and
/// credits, and where its log is.
fn contents(ui: &mut egui::Ui) {
    ui.heading("Voxa");
    ui.label(format!("Version {VERSION}"));
    ui.label("A viewer for MRI scans.");
    ui.add_space(8.0);
    ui.label(INTENDED_USE);
    ui.label(
        RichText::new(theme::DISCLAIMER)
            .strong()
            .color(theme::palette(ui.ctx()).warning),
    );
    ui.add_space(8.0);
    for line in LICENCES {
        ui.label(
            RichText::new(line)
                .small()
                .color(theme::palette(ui.ctx()).quiet_text),
        );
    }
    if let Some(log) = crate::log::path() {
        ui.add_space(8.0);
        ui.label(
            RichText::new(LOG_SAYS)
                .small()
                .color(theme::palette(ui.ctx()).quiet_text),
        );
        ui.horizontal(|ui| {
            let path = log.display().to_string();
            ui.label(RichText::new(&path).small().monospace());
            if ui.small_button("Copy").clicked() {
                ui.ctx().copy_text(path);
            }
        });
    }
}

/// Show every line of the notices while `open` is true, in a window that scrolls both ways,
/// with its bars in sight.
/// Only the lines in view are laid out, so the window stays quick however long the notices.
fn notices(ctx: &egui::Context, open: &mut bool) {
    // Wide enough for the 78 columns most of the texts are written to.
    let window = egui::Window::new(NOTICES_TITLE);
    floating::over(window, ctx)
        .id(notices_id())
        .open(open)
        .collapsible(false)
        .default_size([700.0, 440.0])
        .show(ctx, |ui| {
            scrolled::solid_bars(ui);
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
    use crate::ui::scrolled::{Scrolled, bar_drawn};

    /// The name of the button in the About window that opens the notices.
    const BUTTON: &str = "Licences and notices…";

    #[test]
    fn the_window_shows_and_can_be_left_open() {
        let ctx = egui::Context::default();
        let mut open = true;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(ui.ctx(), &mut open);
        });
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
        assert!(open);
    }

    /// The About window in a Voxa window `width` by `height` points large, in Voxa's look:
    /// what its text held and showed, whether the text's bar is drawn, where the window is,
    /// and where the button that opens the notices is, as a screen reader is told.
    fn drawn_in(width: f32, height: f32) -> (Scrolled, bool, egui::Rect, egui::Rect) {
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
        let ctx = egui::Context::default();
        theme::apply(&ctx, theme::ThemeChoice::Dark);
        ctx.enable_accesskit();
        let (mut scrolled, mut bar, mut button) = (None, false, None);
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..egui::RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                scrolled = show(ui.ctx(), &mut true).map(|text| Scrolled::of(&text));
            });
            // No renderer is behind these passes to take the font texture.
            output.textures_delta.clear();
            let shown = scrolled.map_or(egui::Rect::NOTHING, |scrolled| scrolled.shown);
            bar = bar_drawn(&output.shapes, shown);
            let update = output.platform_output.accesskit_update;
            let nodes = update.map(|update| update.nodes).unwrap_or_default();
            let named = nodes.iter().find(|(_, node)| node.label() == Some(BUTTON));
            button = named.and_then(|(_, node)| node.bounds());
        }
        let window = ctx.memory(|memory| memory.area_rect(id()));
        let window = window.expect("the window is shown");
        let button = button.expect("the button is there");
        let corner = |x: f64, y: f64| egui::pos2(narrowed(x), narrowed(y));
        let button =
            egui::Rect::from_min_max(corner(button.x0, button.y0), corner(button.x1, button.y1));
        (scrolled.expect("the text is laid out"), bar, window, button)
    }

    /// A place in a window, as a screen reader is told it, in the interface's points.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a place in a window of a usual size, far inside what an f32 holds"
    )]
    fn narrowed(at: f64) -> f32 {
        at as f32
    }

    #[test]
    fn the_button_for_the_notices_is_in_sight_and_the_bar_of_the_text_is_drawn() {
        // The smallest window with the interface at 150%, and a usual one.
        for (width, height) in [(427.0, 320.0), (900.0, 600.0)] {
            let (scrolled, bar, window, button) = drawn_in(width, height);
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
            assert!(
                window.contains_rect(button) && screen.contains_rect(button),
                "the button at {button:?} is out of sight of the window at {window:?}"
            );
            assert!(scrolled.is_cut(), "the text is whole at {width}x{height}");
            assert!(bar, "no bar beside the text at {width}x{height}");
        }
    }

    #[test]
    fn the_window_fits_in_a_short_window() {
        // The smallest window, with the interface drawn at 150%.
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(427.0, 320.0));
        let ctx = egui::Context::default();
        theme::apply(&ctx, theme::ThemeChoice::Dark);
        let mut open = true;
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..egui::RawInput::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                show(ui.ctx(), &mut open);
            });
            // No renderer is behind these passes to take the font texture.
            output.textures_delta.clear();
        }
        let shown = ctx.memory(|memory| memory.area_rect(id()));
        let shown = shown.expect("the window is shown");
        assert!(screen.contains_rect(shown), "{shown:?}");
    }

    #[test]
    fn the_place_of_the_log_is_said_in_an_information_notice() {
        let path = std::path::Path::new("logs").join("voxa.log");
        let said = log_notice(Some(&path));
        assert_eq!(said.kind, NoticeKind::Info);
        assert!(
            said.text.contains(&path.display().to_string()),
            "{}",
            said.text
        );
        assert_eq!(said.key, Some("log"));
        let none = log_notice(None);
        assert_eq!(none.kind, NoticeKind::Info);
        assert!(none.text.contains("not keeping a log"), "{}", none.text);
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
