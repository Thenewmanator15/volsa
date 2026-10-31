//! The 3D tab: what the 3D scene offers. The looks (Surface, VR, MIP, Parts), the sides the
//! patient is seen from, the cuts, whether the slice shown in 2D is drawn in the scene, the
//! saved views, and saving a picture or a turntable.

use egui::{RichText, Ui};
use voxa_render::{Look, MAX_CUTS, NamedView};

use super::super::commands::{self, Request, menus};
use super::super::scene::{export, views};
use super::super::theme;
use super::{offer, row, section};
use crate::action::Action;
use crate::labels::look_drawn;
use crate::state::AppState;

/// The looks offered, with the label and hover text of each button. The labels are the names
/// radiology uses; the hover text says what each means. The list of actions takes its entries
/// for the looks from these.
pub const LOOKS: [(Look, &str, &str); 3] = [
    (
        Look::Surface,
        "Surface",
        "A solid, lit surface where the tissue begins",
    ),
    (
        Look::Vr,
        "VR",
        "Volume rendering: see-through, brighter tissue more solid",
    ),
    (
        Look::Mip,
        "MIP",
        "Maximum-intensity projection: the brightest value on each line of sight",
    ),
];

/// What the Parts look is, and why it cannot be chosen while there are no labels.
pub const PARTS_HINT: &str = "The labelled parts of the head as solids, each in its own colour";
pub const PARTS_NEED_LABELS: &str = "Open labels in the Labels tab to see them as solids here.";

/// Said at the top of the tab while the arrangement shown has no 3D view, which its controls act
/// on all the same.
pub const NO_3D_VIEW: &str = "Choose 3D, Planes or Slices + 3D to see this.";

/// The sides the patient can be seen from, with what each button says, its letter and a word, and
/// what it means, from which the list of actions takes its entries for them.
pub const VIEWS: [(NamedView, &str, &str); 6] = [
    (NamedView::Anterior, "A front", "From the front (anterior)"),
    (NamedView::Posterior, "P back", "From behind (posterior)"),
    (NamedView::Left, "L left", "From the patient's left"),
    (NamedView::Right, "R right", "From the patient's right"),
    (
        NamedView::Superior,
        "S top",
        "From above the head (superior)",
    ),
    (
        NamedView::Inferior,
        "I bottom",
        "From below the feet (inferior)",
    ),
];

/// The looks to choose between, each by the action that chooses it: the three of the scan,
/// and then the labelled parts as solids, which can be chosen when there are labels on this
/// series to draw.
const LOOK_IDS: [(Look, &str); 4] = [
    (Look::Surface, "scene.look.surface"),
    (Look::Vr, "scene.look.vr"),
    (Look::Mip, "scene.look.mip"),
    (Look::Parts, "scene.look.parts"),
];

/// The sides the patient can be seen from, by their actions, in the order of [`VIEWS`].
const SIDE_IDS: [&str; 6] = [
    "scene.side.anterior",
    "scene.side.posterior",
    "scene.side.left",
    "scene.side.right",
    "scene.side.superior",
    "scene.side.inferior",
];

/// The actions that remove each cut there can be, the first cut's first.
const UNCUT_IDS: [&str; MAX_CUTS] = ["scene.uncut.1", "scene.uncut.2", "scene.uncut.3"];

/// The looks to choose between. The look in use is not asked for again.
fn looks(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>, asked: &mut Vec<Request>) {
    let drawn = look_drawn(state);
    for (look, id) in LOOK_IDS {
        let chosen = drawn == look;
        let pressed = commands::button(ui, id, state, Some(chosen));
        if let Some(does) = pressed.filter(|_| !chosen) {
            asked.extend(does.run(state, actions));
        }
    }
}

/// Draw the 3D tab: the looks, the sides to see the patient from, the cuts, whether the slice
/// shown in 2D is drawn in the scene, the saved views, and saving to a file. Changes of state
/// go onto `actions`; what its buttons ask of the window, such as saving a picture, is given
/// back.
pub fn show(ui: &mut Ui, state: &AppState, actions: &mut Vec<Action>) -> Vec<Request> {
    let mut asked = Vec::new();
    // A scan with no 3D view says why; otherwise an arrangement without one says how to get it.
    let why = commands::no_3d_view(state);
    let said = why.map(std::borrow::Cow::into_owned);
    let said = said.or_else(|| {
        let shown = state.pictures.given(&state.scene.status).has_scene();
        (!shown).then(|| NO_3D_VIEW.to_owned())
    });
    if let Some(said) = said {
        let quiet = theme::palette(ui.ctx()).quiet_text;
        ui.label(RichText::new(said).small().color(quiet));
    }
    section(ui, "Look");
    row(ui, |ui| looks(ui, state, actions, &mut asked));
    section(ui, "Seen from");
    row(ui, |ui| {
        for id in SIDE_IDS {
            offer(ui, id, state, None, actions, &mut asked);
        }
    });
    section(ui, "Cuts");
    row(ui, |ui| {
        offer(ui, "scene.cut", state, None, actions, &mut asked);
        for id in UNCUT_IDS.iter().take(state.scene.cuts.len()) {
            offer(ui, id, state, None, actions, &mut asked);
        }
    });
    section(ui, "Slice");
    row(ui, |ui| {
        // The slice is a picture of the scan: it is not drawn among the labelled solids.
        let shown = Some(state.scene.slice_shown);
        offer(ui, "scene.slice", state, shown, actions, &mut asked);
    });
    section(ui, "Views and pictures");
    let saving = row(ui, |ui| {
        views::menu(ui, state, (actions, &mut asked));
        let export = &menus::EXPORT;
        let menu =
            commands::menu_button(ui, export, export.label, |ui| export::contents(ui, state));
        menu.inner.flatten()
    });
    asked.extend(saving.map(Request::SaveScene));
    asked
}

#[cfg(test)]
mod tests;
