//! The strip above the pictures: the four ways of arranging them, how many viewports while the
//! slices are among them, and, at its right end, which slice is shown and of which volume.
//! Its buttons are actions of the list of actions, so an arrangement the scan cannot have is
//! offered greyed, saying why.

use egui::{Align, Label, Layout, Rect, RichText, Ui, UiBuilder, vec2};

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

/// Draw the strip in `rect`: the arrangements, the one shown chosen, with the numbers of
/// viewports beside the slices while they are shown, and at the right end where the slice
/// shown is. What is chosen already is not asked for again. Changes of state go onto
/// `actions`, and what the buttons ask of the window is given back.
pub fn show(ui: &mut Ui, rect: Rect, state: &AppState, actions: &mut Vec<Action>) -> Vec<Request> {
    let inside = rect.shrink2(vec2(4.0, 0.0));
    let builder = UiBuilder::new()
        .max_rect(inside)
        .layout(Layout::left_to_right(Align::Center));
    let mut strip = ui.new_child(builder);
    strip.spacing_mut().item_spacing.x = 4.0;
    let shown = state.pictures.given(&state.scene.status);
    let mut asked = Vec::new();
    let mut offer = |ui: &mut Ui, id: &str, chosen: bool| {
        let pressed = commands::button(ui, id, state, Some(chosen));
        if let Some(does) = pressed.filter(|_| !chosen) {
            asked.extend(does.run(state, actions));
        }
    };
    for (pictures, id) in ARRANGEMENTS {
        offer(&mut strip, id, shown == pictures);
        if pictures == Pictures::Slices && shown.has_slices() {
            for (layout, id) in LAYOUTS {
                offer(&mut strip, id, state.layout == layout);
            }
            strip.separator();
        }
    }
    if let Some(said) = where_you_are(state) {
        strip.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let words = RichText::new(said).small().color(theme::QUIET_TEXT);
            ui.add(Label::new(words).truncate());
        });
    }
    asked
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
