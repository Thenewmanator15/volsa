//! The help, opened with `?`: short topics for people looking at their own scans, above the list
//! of every key and what the mouse does.

use egui::RichText;
use egui::scroll_area::ScrollAreaOutput;

use super::{commands, floating, scrolled, theme};

#[cfg(test)]
mod drawn_tests;
#[cfg(test)]
mod open_tests;
mod topics;
#[cfg(test)]
mod wording_sources;
#[cfg(test)]
mod wording_tests;

pub use topics::TOPICS;

/// The heading over the topics.
const TOPICS_HEADING: &str = "Topics";
/// The heading over the keys and what the mouse does.
const KEYS_HEADING: &str = "Keys and mouse";

/// Where the topic the help is to open at is kept until the window has drawn it.
const AT: &str = "voxa: the help topic to open at";

/// Do what asking for the help does: with no topic it opens the help, or shuts it if it is
/// open; with a topic it opens the help at that topic, whether it was open or not.
pub fn ask(ctx: &egui::Context, open: &mut bool, topic: Option<&'static str>) {
    match topic {
        None => *open = !*open,
        Some(topic) => {
            *open = true;
            ctx.data_mut(|data| data.insert_temp(egui::Id::new(AT), topic));
        }
    }
}

/// The topic the help is to open at, if one was asked for and is not yet drawn.
fn asked_for(ctx: &egui::Context) -> Option<&'static str> {
    ctx.data(|data| data.get_temp(egui::Id::new(AT)))
}

/// Forget the topic asked for: it is drawn, or the help was shut before it could be.
fn forget_the_topic(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<&'static str>(egui::Id::new(AT)));
}

/// What the mouse does over a viewport, for the same window.
pub const MOUSE: [(&str, &str); 15] = [
    ("Wheel", "Move through the slices"),
    ("Ctrl + wheel", "Zoom about the pointer"),
    ("Middle drag", "Move the picture"),
    (
        "Right drag",
        "Change the window: across for width, down for level",
    ),
    ("Double-click", "Fit the whole picture in the viewport"),
    (
        "Click or drag, on a slice",
        "Draw with the tool in hand, in the viewport chosen",
    ),
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
        "Click Label this scan, in the Tools tab",
        "Label the scan with Voxa's own networks. The menu beside it chooses how",
    ),
    (
        "Click Stop, in the Tools tab",
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
pub(super) fn id() -> egui::Id {
    egui::Id::new("voxa: help")
}

/// Show the help while `open` is true; closing it sets `open` to false. The topics come first,
/// each closed until it is clicked, except the one asked for (see [`ask`]), which is opened and
/// brought into sight; then the keys and the mouse. In a window too small to hold it all, it
/// scrolls, with its bar in sight; in a narrow one, what each key does follows it. Gives what
/// egui said of its list, while it is shown.
pub fn show(ctx: &egui::Context, open: &mut bool) -> Option<ScrollAreaOutput<()>> {
    if !*open {
        // A topic asked for as the help was shut is not for the next time it is opened.
        forget_the_topic(ctx);
    }
    let at = asked_for(ctx);
    let room = floating::area(ctx).size();
    let wide = (room.x - AROUND).clamp(AROUND, WIDEST);
    let window = egui::Window::new("Help");
    let mut in_sight = false;
    let shown = floating::over(window, ctx)
        .id(id())
        .open(open)
        .collapsible(false)
        .auto_sized()
        .show(ctx, |ui| {
            ui.set_width(wide);
            // The bar is always drawn, so that a list cut short does not look whole.
            scrolled::solid_bars(ui);
            let most = (room.y - 2.0 * AROUND).max(AROUND);
            scrolled::up_and_down(most).show(ui, |ui| {
                in_sight = topics(ui, at);
                ui.add_space(8.0);
                ui.label(RichText::new(KEYS_HEADING).strong());
                if wide >= BESIDE {
                    side_by_side(ui);
                } else {
                    run_on(ui);
                }
            })
        });
    if in_sight {
        forget_the_topic(ctx);
    }
    shown.and_then(|window| window.inner)
}

/// The topics, each a heading that opens to its words. The topic `at` is opened, and brought
/// to the top of the list once the window has its size, which it learns in a pass of its own
/// that is not seen. Gives whether that was done.
fn topics(ui: &mut egui::Ui, at: Option<&str>) -> bool {
    let mut brought = false;
    ui.label(RichText::new(TOPICS_HEADING).strong());
    for topic in &TOPICS {
        let asked = at == Some(topic.id);
        let header = egui::CollapsingHeader::new(topic.title)
            .id_salt(("voxa: help topic", topic.id))
            .open(asked.then_some(true))
            .show(ui, |ui| {
                ui.add(egui::Label::new(topic.body).wrap());
            });
        if asked && !ui.is_sizing_pass() {
            let jump = egui::style::ScrollAnimation::none();
            header
                .header_response
                .scroll_to_me_animation(Some(egui::Align::TOP), jump);
            brought = true;
        }
    }
    brought
}

/// Every key Voxa answers to, from the list of actions, with what it does: each action that
/// has a key, in the list's order.
pub fn keys_listed() -> Vec<(&'static str, &'static str)> {
    let keyed = commands::all().iter();
    keyed
        .filter_map(|command| Some((command.key?.shown, command.name)))
        .collect()
}

/// What the reference lists, in order: the keys, then what the mouse does.
fn listed() -> impl Iterator<Item = (&'static str, &'static str)> {
    keys_listed().into_iter().chain(MOUSE)
}

/// A key or mouse action, as the reference names it.
fn key(ui: &egui::Ui, input: &str) -> RichText {
    let accent = theme::palette(ui.ctx()).accent;
    RichText::new(input).monospace().color(accent)
}

/// Each key with what it does beside it, in two columns.
fn side_by_side(ui: &mut egui::Ui) {
    egui::Grid::new("shortcuts")
        .num_columns(2)
        .spacing([24.0, 6.0])
        .show(ui, |ui| {
            for (input, does) in listed() {
                ui.label(key(ui, input));
                ui.add(egui::Label::new(does).wrap());
                ui.end_row();
            }
        });
}

/// Each key followed by what it does, on as many lines as that takes, for a narrow window.
fn run_on(ui: &mut egui::Ui) {
    let keys = egui::TextStyle::Monospace.resolve(ui.style());
    let words = egui::TextStyle::Body.resolve(ui.style());
    let colour = ui.visuals().text_color();
    for (input, does) in listed() {
        let mut line = egui::text::LayoutJob::default();
        let named = egui::TextFormat::simple(keys.clone(), theme::palette(ui.ctx()).accent);
        line.append(input, 0.0, named);
        line.append(does, 12.0, egui::TextFormat::simple(words.clone(), colour));
        ui.add(egui::Label::new(line).wrap());
    }
}

/// Every piece of text the reference draws, open in a window `width` by `height` points
/// large, after it has had a frame to lay itself out.
#[cfg(test)]
pub fn drawn_words(width: f32, height: f32) -> Vec<String> {
    fn words_of(shape: &egui::Shape, into: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => into.push(text.galley.text().to_owned()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| words_of(shape, into)),
            _ => {}
        }
    }
    let ctx = egui::Context::default();
    theme::apply(&ctx, theme::ThemeChoice::Dark);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut words = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            ..egui::RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let _ = show(ui.ctx(), &mut true);
        });
        // No renderer is behind this pass to take the font texture.
        output.textures_delta.clear();
        words.clear();
        for clipped in &output.shapes {
            words_of(&clipped.shape, &mut words);
        }
    }
    words
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
        assert_eq!(keys_listed().len() + MOUSE.len(), 43);
        let parts = MOUSE
            .iter()
            .find(|(input, _)| input.contains("labelled part"));
        let (_, does) = parts.expect("what a click on a part does is listed");
        assert!(does.contains("Select") && does.contains("name"), "{does}");
        let label = MOUSE
            .iter()
            .find(|(input, _)| input.contains("Label this scan"));
        let (where_it_is, does) = label.expect("what the button that labels does is listed");
        assert!(where_it_is.ends_with("in the Tools tab"), "{where_it_is}");
        assert!(does.contains("Voxa's own networks"), "{does}");
        let stop = MOUSE.iter().find(|(input, _)| input.contains("Stop"));
        let (where_it_is, does) = stop.expect("what Stop does is listed");
        assert!(where_it_is.ends_with("in the Tools tab"), "{where_it_is}");
        assert!(does.contains("Stop labelling"), "{does}");
        let in_3d = MOUSE
            .iter()
            .filter(|(input, _)| input.ends_with("in 3D"))
            .count();
        assert_eq!(in_3d, 5, "what the mouse does in the 3D view is listed too");
    }

    #[test]
    fn the_list_of_keys_has_the_ones_for_measuring_and_the_screen() {
        let listed = keys_listed();
        let does = |key: &str| listed.iter().find(|(shown, _)| *shown == key);
        let does = |key| does(key).map(|(_, does)| *does);
        for key in [
            "Ctrl+Z",
            "Ctrl+Y or Ctrl+Shift+Z",
            "M",
            "B",
            "F11",
            "?",
            "Ctrl+K or /",
        ] {
            assert!(does(key).is_some(), "{key}");
        }
        let escape = does("Esc").expect("listed");
        assert!(escape.contains("tool"), "Esc puts the tool down: {escape}");
        for key in ["Ctrl+=", "Ctrl+-", "Ctrl+0"] {
            let size = does(key).expect("listed");
            assert!(size.contains("interface"), "{key}: {size}");
        }
        let pages = does("Page Up").expect("listed");
        assert!(pages.contains("Ten slices"), "{pages}");
    }

    /// The reference shown in a Voxa window `width` by `height` points large, in Voxa's
    /// look: what its list held and showed, whether the list's bar is drawn, and where the
    /// reference is.
    fn drawn_in(width: f32, height: f32) -> (Scrolled, bool, egui::Rect) {
        let ctx = egui::Context::default();
        theme::apply(&ctx, theme::ThemeChoice::Dark);
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
        let shown = ctx.memory(|memory| memory.area_rect(id()));
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
