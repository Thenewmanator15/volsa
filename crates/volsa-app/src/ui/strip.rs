//! The strip above the pictures: the four ways of arranging them, how many viewports while the
//! slices are among them, and, at its right end, which slice is shown and of which volume.
//! Its buttons are actions of the list of actions, so an arrangement the scan cannot have is
//! offered greyed, saying why.
//!
//! Where the middle of the window is too narrow for all the buttons, they fold into one menu
//! that holds them, so that every arrangement can still be chosen and none is cut off.

use egui::containers::menu::MenuButton;
use egui::{
    Align, Atom, Button, Id, Label, Layout, Rect, RichText, Shape, Stroke, TextStyle, TextWrapMode,
    Ui, UiBuilder, Vec2, WidgetText, vec2,
};

use super::commands::{self, Request};
use super::{theme, volumes};
use crate::action::Action;
use crate::pictures::Pictures;
use crate::state::{self, AppState};

/// The arrangements, each by the action that chooses it, in the order the strip offers them.
const ARRANGEMENTS: [(Pictures, &str); 4] = [
    (Pictures::Slices, "pictures.slices"),
    (Pictures::SlicesAndScene, "pictures.slices3d"),
    (Pictures::Planes, "pictures.planes"),
    (Pictures::Scene, "pictures.scene"),
];

/// The numbers of viewports, each by the action that chooses it.
const LAYOUTS: [(state::Layout, &str); 3] = [
    (state::Layout::One, "layout.one"),
    (state::Layout::TwoAcross, "layout.two"),
    (state::Layout::TwoByTwo, "layout.four"),
];

/// What the menu that holds the buttons of a narrow strip is for, after the arrangement it
/// shows on itself.
const ARRANGE_MENU: &str = "Arrange the pictures";
/// How wide the caret on the menu is, and how high.
const CARET: Vec2 = vec2(8.0, 5.0);
/// The room the buttons are given beyond their own widths before they are folded into the menu,
/// in points, so that rounding never cuts the last of them short.
const SPARE: f32 = 8.0;
/// The space between the strip's items, in points.
const GAP: f32 = 4.0;

/// What a screen reader calls the menu of a narrow strip while `shown` is the arrangement: the
/// words written on it, then what it is for.
pub fn menu_name(shown: Pictures) -> String {
    format!("{}, {ARRANGE_MENU}", label_of(shown))
}

/// The words on the button that chooses `pictures`.
fn label_of(pictures: Pictures) -> &'static str {
    let found = ARRANGEMENTS.iter().find(|(one, _)| *one == pictures);
    found
        .and_then(|(_, id)| commands::find(id))
        .map_or("Pictures", |command| command.label)
}

/// Draw the strip in `rect`: the arrangements, the one shown chosen, with the numbers of
/// viewports beside the slices while they are shown, and at the right end where the slice
/// shown is. What is chosen already is not asked for again. Changes of state go onto
/// `actions`, and what the buttons ask of the window is given back.
pub fn show(ui: &mut Ui, rect: Rect, state: &AppState, actions: &mut Vec<Action>) -> Vec<Request> {
    let inside = rect.shrink2(vec2(GAP, 0.0));
    let builder = UiBuilder::new()
        .max_rect(inside)
        .layout(Layout::left_to_right(Align::Center));
    let mut strip = ui.new_child(builder);
    strip.spacing_mut().item_spacing.x = GAP;
    let shown = state.pictures.given(&state.scene.status);
    let mut asked = Vec::new();
    let mut offer = |ui: &mut Ui, id: &str, chosen: bool| {
        let pressed = commands::button(ui, id, state, Some(chosen));
        if let Some(does) = pressed.filter(|_| !chosen) {
            asked.extend(does.run(state, actions));
        }
    };
    if inside.width() < width_of_buttons(&strip, shown) + SPARE {
        folded(&mut strip, shown, state, &mut offer);
    } else {
        for (pictures, id) in ARRANGEMENTS {
            offer(&mut strip, id, shown == pictures);
            if pictures == Pictures::Slices && shown.has_slices() {
                for (layout, id) in LAYOUTS {
                    offer(&mut strip, id, state.layout == layout);
                }
                strip.separator();
            }
        }
    }
    // Which slice is shown describes the slices, so it is said only while they are in sight.
    if let Some(said) = where_you_are(state).filter(|_| shown.has_slices()) {
        strip.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // The strip lies on the pictures' ground, which is dark in both themes.
            let words = RichText::new(said).small().color(theme::QUIET_TEXT);
            ui.add(Label::new(words).truncate());
        });
    }
    asked
}

/// How wide the strip's buttons are laid out side by side for the arrangement `shown`: the
/// arrangements, and the numbers of viewports with their separator while the slices are shown.
fn width_of_buttons(ui: &Ui, shown: Pictures) -> f32 {
    let padding = 2.0 * ui.spacing().button_padding.x;
    let mut ids: Vec<&str> = ARRANGEMENTS.iter().map(|(_, id)| *id).collect();
    if shown.has_slices() {
        ids.extend(LAYOUTS.iter().map(|(_, id)| *id));
    }
    let words = |id: &&str| commands::find(id).map_or(0.0, |command| wide(ui, command.label));
    let buttons: f32 = ids.iter().map(words).sum::<f32>() + padding * to_f32(ids.len());
    // A gap after every item, and the separator, which is a gap wide in itself.
    let items = ids.len() + usize::from(shown.has_slices());
    buttons + GAP * to_f32(items + 1)
}

/// A count of a few items, as points.
#[expect(
    clippy::cast_precision_loss,
    reason = "a count of a handful of buttons, far inside what an f32 holds"
)]
fn to_f32(count: usize) -> f32 {
    count as f32
}

/// The width of `text` laid out on one line as a button writes it.
fn wide(ui: &Ui, text: &str) -> f32 {
    let style = TextStyle::Button;
    let text = WidgetText::from(RichText::new(text).text_style(style.clone()));
    let laid = text.into_galley(ui, Some(TextWrapMode::Extend), f32::INFINITY, style);
    laid.size().x
}

/// The buttons folded into one menu, which shows the arrangement now shown and a caret, and
/// holds each arrangement and, while the slices are shown, each number of viewports.
fn folded(
    ui: &mut Ui,
    shown: Pictures,
    state: &AppState,
    offer: &mut impl FnMut(&mut Ui, &str, bool),
) {
    let room = Id::new("volsa: the caret of the menu of arrangements");
    let button = Button::new((label_of(shown), Atom::custom(room, CARET)));
    let contents = |ui: &mut Ui| {
        for (pictures, id) in ARRANGEMENTS {
            offer(ui, id, shown == pictures);
        }
        if shown.has_slices() {
            ui.separator();
            for (layout, id) in LAYOUTS {
                offer(ui, id, state.layout == layout);
            }
        }
    };
    let (response, _) = MenuButton::from_button(button).ui(ui, contents);
    let name = menu_name(shown);
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    let response = response.on_hover_text(ARRANGE_MENU);
    let colour = ui.style().interact(&response).fg_stroke.color;
    let at = Rect::from_center_size(
        response.rect.right_center() - vec2(ui.spacing().button_padding.x + CARET.x / 2.0, 0.0),
        CARET,
    );
    let corners = vec![at.left_top(), at.right_top(), at.center_bottom()];
    ui.painter()
        .add(Shape::convex_polygon(corners, colour, Stroke::NONE));
}

/// Which slice the active viewport shows, of how many, and of a scan of several volumes which
/// volume: "slice 88 of 176 · volume 2 of 5 · b 1000". `None` while it shows nothing.
fn where_you_are(state: &AppState) -> Option<String> {
    let viewport = state.viewports.get(state.active)?;
    let listed = state.series(viewport.series.as_ref()?)?;
    let slice = format!("slice {} of {}", viewport.slice + 1, listed.slices.len());
    let volume = volumes::label(&listed.info).map(|volume| {
        let mut letters = volume.chars();
        let first = letters
            .next()
            .map(|first| first.to_lowercase().collect::<String>());
        first.unwrap_or_default() + letters.as_str()
    });
    Some(match volume {
        Some(volume) => format!("{slice} · {volume}"),
        None => slice,
    })
}

#[cfg(test)]
mod tests;
