//! What the About window and the notices window say, as a screen reader is told of them.

use super::{INTENDED_USE, LICENCES, LOG_SAYS, NOTICE, THIRD_PARTY, notices, open_notices, show};
use crate::ui::theme;

/// The name of the button in the About window that opens the notices.
const BUTTON: &str = "Licences and notices…";
/// The title of the notices window.
const TITLE: &str = "Licences and notices";
/// The page that says what Voxa is for, which comes in a download of Voxa too.
const INTENDED_USE_PAGE: &str = include_str!("../../../../../docs/intended-use.md");

/// The words of a text with one space between each two, however its lines were wrapped.
fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn the_about_window_says_what_voxa_is_for_as_the_page_on_it_does() {
    let ctx = egui::Context::default();
    let says = about_says(&ctx);
    for wanted in [INTENDED_USE, theme::DISCLAIMER] {
        assert!(
            says.iter().any(|said| said == wanted),
            "the About window does not say {wanted:?}:\n{says:?}"
        );
    }
    // What the window says, and then the disclaimer, is the page's paragraph word for word.
    let paragraph = format!("{INTENDED_USE} {}", theme::DISCLAIMER);
    assert!(
        words(INTENDED_USE_PAGE).contains(&paragraph),
        "docs/intended-use.md does not hold {paragraph:?}"
    );
}

/// Every label's text and every button's name the interface gives a screen reader after a
/// pass of `run`, with `input` for that pass.
fn said(ctx: &egui::Context, input: egui::RawInput, run: impl Fn(&egui::Context)) -> Vec<String> {
    ctx.enable_accesskit();
    let mut output = ctx.run_ui(input, |ui| run(ui.ctx()));
    // No renderer is behind these passes to take the font texture.
    output.textures_delta.clear();
    let update = output.platform_output.accesskit_update;
    let update = update.expect("the interface is described");
    let text =
        |(_, node): &(_, egui::accesskit::Node)| node.value().or(node.label()).map(str::to_owned);
    update.nodes.iter().filter_map(text).collect()
}

/// The point halfway between two edges a screen reader is told of, in the interface's points.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a place in a window of default size, far inside what an f32 holds"
)]
fn middle(one: f64, other: f64) -> f32 {
    f64::midpoint(one, other) as f32
}

/// What the About window says once it has been laid out.
fn about_says(ctx: &egui::Context) -> Vec<String> {
    said(ctx, egui::RawInput::default(), |ctx| show(ctx, &mut true));
    said(ctx, egui::RawInput::default(), |ctx| show(ctx, &mut true))
}

#[test]
fn the_about_window_names_every_typeface_with_its_licence_and_credits_synthseg() {
    let ctx = egui::Context::default();
    let says = about_says(&ctx).join("\n");
    let wanted = [
        theme::DISCLAIMER,
        "either the MIT licence or the Apache License 2.0",
        "Ubuntu Light (Ubuntu Font Licence 1.0)",
        "Hack (MIT, and the Bitstream Vera licence",
        "Noto Emoji (SIL Open Font Licence 1.1)",
        "emoji-icon-font (MIT)",
        "SynthSeg's code (Copyright 2020 Benjamin Billot, Apache License 2.0)",
        "based in part on the work of the Independent JPEG Group",
        "THIRD-PARTY-NOTICES.txt and NOTICE, beside the program, hold the full texts",
        BUTTON,
    ];
    for words in wanted {
        assert!(
            says.contains(words),
            "the About window does not say {words:?}:\n{says}"
        );
    }
    // Not every licence is permissive: one of the libraries is under the Mozilla licence.
    assert!(!says.contains("permissive"), "{says}");
    assert!(LICENCES.iter().all(|line| says.contains(line)));
}

#[test]
fn the_about_window_and_notice_say_microsofts_c_runtime_is_built_in_under_its_own_terms() {
    // The voxa.exe a release offers is built with Microsoft's C runtime inside it, which is
    // not one of the open-source libraries the notices list.
    let ctx = egui::Context::default();
    let says = about_says(&ctx).join("\n");
    for words in [
        "voxa.exe a release offers has Microsoft's C runtime built in",
        "under Microsoft's own terms, not an open-source licence",
    ] {
        assert!(
            says.contains(words),
            "the About window does not say {words:?}:\n{says}"
        );
    }
    let notice = words(NOTICE);
    for said in [
        "has Microsoft's C runtime built into it",
        "under Microsoft's own licence terms",
    ] {
        assert!(notice.contains(said), "NOTICE does not say {said:?}");
    }
}

#[test]
fn what_the_about_window_says_of_the_log_holds_for_a_crash_too() {
    // A crash is logged by its place in the source code, which for a library can be a path
    // on the computer Voxa was built on: so no promise of no file names or no path.
    assert!(!LOG_SAYS.contains("no file names"), "{LOG_SAYS}");
    for said in [
        "nothing about a patient",
        "none of your files' names",
        "A crash is logged by the place in the source code of Voxa, or of a library",
    ] {
        assert!(LOG_SAYS.contains(said), "{LOG_SAYS}");
    }
}

#[test]
fn the_button_in_the_about_window_opens_the_notices_from_their_first_line() {
    let ctx = egui::Context::default();
    let says = about_says(&ctx);
    assert!(!says.iter().any(|said| said == TITLE), "{says:?}");
    // Where the button is, as the screen reader is told.
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| show(ui.ctx(), &mut true));
    output.textures_delta.clear();
    let update = output.platform_output.accesskit_update.expect("described");
    let button = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(BUTTON));
    let bounds = button
        .and_then(|(_, node)| node.bounds())
        .expect("the button has a place");
    let at = egui::pos2(middle(bounds.x0, bounds.x1), middle(bounds.y0, bounds.y1));
    let press = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    let input = |events| egui::RawInput {
        events,
        ..Default::default()
    };
    said(
        &ctx,
        input(vec![egui::Event::PointerMoved(at), press(true)]),
        |ctx| show(ctx, &mut true),
    );
    said(&ctx, input(vec![press(false)]), |ctx| show(ctx, &mut true));
    // The notices window is laid out unseen once, and then shown.
    said(&ctx, egui::RawInput::default(), |ctx| show(ctx, &mut true));
    let says = said(&ctx, egui::RawInput::default(), |ctx| show(ctx, &mut true));
    assert!(says.iter().any(|said| said == TITLE), "{says:?}");
    let first = NOTICE.lines().next().unwrap_or_default();
    assert!(
        !first.is_empty() && says.iter().any(|said| said == first),
        "{says:?}"
    );
}

#[test]
fn the_notices_are_the_files_beside_the_program() {
    assert!(NOTICE.contains(theme::DISCLAIMER));
    for words in [
        "SynthSeg",
        "Copyright 2020 Benjamin Billot",
        "https://github.com/BBillot/SynthSeg",
    ] {
        assert!(NOTICE.contains(words), "NOTICE does not say {words}");
    }
    assert!(THIRD_PARTY.starts_with("THIRD-PARTY NOTICES FOR VOXA"));
    for typeface in [
        "Hack Regular",
        "Ubuntu Light",
        "Noto Emoji Regular",
        "emoji-icon-font",
    ] {
        assert!(
            THIRD_PARTY.contains(&format!("\n  {typeface}: ")),
            "{typeface}"
        );
    }
    // Once opened, the notices window stays open when the About window is closed.
    let ctx = egui::Context::default();
    open_notices(&ctx);
    said(&ctx, egui::RawInput::default(), |ctx| show(ctx, &mut false));
    let says = said(&ctx, egui::RawInput::default(), |ctx| show(ctx, &mut false));
    assert!(says.iter().any(|said| said == TITLE), "{says:?}");
    // The window can be shown on its own.
    let ctx = egui::Context::default();
    said(&ctx, egui::RawInput::default(), |ctx| {
        notices(ctx, &mut true);
    });
    let says = said(&ctx, egui::RawInput::default(), |ctx| {
        notices(ctx, &mut true);
    });
    assert!(says.iter().any(|said| said == TITLE), "{says:?}");
}
