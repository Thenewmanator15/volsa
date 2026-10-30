//! The 3D scene: its looks, the sides the patient is seen from, the cuts, the slice drawn in
//! it, saving the view, and saving it as a picture or a turntable.

use voxa_render::{Look, MAX_CUTS, NamedView};

use super::super::{Command, Does, Place, Request, Tab};
use super::PLAIN;
use crate::action::Action;
use crate::labels::{look_drawn, on_this_series};
use crate::scene::SceneStatus;
use crate::state::AppState;
use crate::ui::scene::export;
use crate::ui::scene_bar::{LOOKS, PARTS_HINT, PARTS_NEED_LABELS, VIEWS};
use crate::views::ViewsAction;

/// Why the slice shown in 2D cannot be drawn in the scene while it shows the Parts look.
const NO_SLICE_AMONG_PARTS: &str =
    "The slice is a picture of the scan: it is not drawn among the labelled parts";

/// The words each side the patient can be seen from is found by.
const SIDE: &[&str] = &["side", "view", "orientation", "camera"];

/// The words each button that removes a cut is found by.
const UNCUT: &[&str] = &["remove", "uncut", "cut"];

/// Why there is nothing to save of the scene yet.
fn not_ready(state: &AppState) -> Option<&'static str> {
    let there = state.scene.status == SceneStatus::Ready;
    (!there).then_some("There is no 3D view to save yet")
}

/// Why a cut cannot be removed: there are fewer than `cut` cuts.
fn no_cut(state: &AppState, cut: usize, why: &'static str) -> Option<&'static str> {
    (state.scene.cuts.len() < cut).then_some(why)
}

/// The entries.
pub(super) const COMMANDS: &[Command] = &[
    Command {
        id: "scene.look.surface",
        name: "Show the scan as a surface",
        label: LOOKS[0].1,
        explains: LOOKS[0].2,
        words: &["surface", "solid", "shaded", "3D"],
        place: Place::Tab(Tab::Scene),
        does: Does::Act(|_| Some(Action::SceneLook(Look::Surface))),
        ..PLAIN
    },
    Command {
        id: "scene.look.vr",
        name: "Show the scan by volume rendering",
        label: LOOKS[1].1,
        explains: LOOKS[1].2,
        words: &["vr", "volume rendering", "see-through", "3D"],
        place: Place::Tab(Tab::Scene),
        does: Does::Act(|_| Some(Action::SceneLook(Look::Vr))),
        ..PLAIN
    },
    Command {
        id: "scene.look.mip",
        name: "Show the brightest value along each line of sight",
        label: LOOKS[2].1,
        explains: LOOKS[2].2,
        words: &[
            "mip",
            "maximum intensity projection",
            "brightest",
            "vessels",
        ],
        place: Place::Tab(Tab::Scene),
        does: Does::Act(|_| Some(Action::SceneLook(Look::Mip))),
        ..PLAIN
    },
    Command {
        id: "scene.look.parts",
        name: "Show the labelled parts as solids",
        label: "Parts",
        explains: PARTS_HINT,
        words: &["parts", "labels", "segmentation", "solids"],
        place: Place::Tab(Tab::Scene),
        why_not: |state| (!on_this_series(state)).then_some(PARTS_NEED_LABELS),
        does: Does::Act(|_| Some(Action::SceneLook(Look::Parts))),
        ..PLAIN
    },
    Command {
        id: "scene.side.anterior",
        name: VIEWS[0].2,
        label: VIEWS[0].1,
        words: SIDE,
        place: Place::Tab(Tab::Scene),
        does: Does::Act(|_| Some(Action::SceneView(NamedView::Anterior))),
        ..PLAIN
    },
    Command {
        id: "scene.side.posterior",
        name: VIEWS[1].2,
        label: VIEWS[1].1,
        words: SIDE,
        place: Place::Tab(Tab::Scene),
        does: Does::Act(|_| Some(Action::SceneView(NamedView::Posterior))),
        ..PLAIN
    },
    Command {
        id: "scene.side.left",
        name: VIEWS[2].2,
        label: VIEWS[2].1,
        words: SIDE,
        place: Place::Tab(Tab::Scene),
        does: Does::Act(|_| Some(Action::SceneView(NamedView::Left))),
        ..PLAIN
    },
    Command {
        id: "scene.side.right",
        name: VIEWS[3].2,
        label: VIEWS[3].1,
        words: SIDE,
        place: Place::Tab(Tab::Scene),
        does: Does::Act(|_| Some(Action::SceneView(NamedView::Right))),
        ..PLAIN
    },
    Command {
        id: "scene.side.superior",
        name: VIEWS[4].2,
        label: VIEWS[4].1,
        words: SIDE,
        place: Place::Tab(Tab::Scene),
        does: Does::Act(|_| Some(Action::SceneView(NamedView::Superior))),
        ..PLAIN
    },
    Command {
        id: "scene.side.inferior",
        name: VIEWS[5].2,
        label: VIEWS[5].1,
        words: SIDE,
        place: Place::Tab(Tab::Scene),
        does: Does::Act(|_| Some(Action::SceneView(NamedView::Inferior))),
        ..PLAIN
    },
    Command {
        id: "scene.cut",
        name: "Cut the volume open",
        label: "Cut",
        explains: "Straight across the view; drag the cut's handle to move it",
        words: &["cut", "clip", "section", "open"],
        place: Place::Tab(Tab::Scene),
        why_not: |state| (state.scene.cuts.len() >= MAX_CUTS).then_some("Three cuts at most"),
        does: Does::Act(|_| Some(Action::CutAdd)),
        ..PLAIN
    },
    Command {
        id: "scene.uncut.1",
        name: "Remove cut 1",
        label: "Remove cut 1",
        explains: "Put back what this cut took away",
        words: UNCUT,
        place: Place::Tab(Tab::Scene),
        why_not: |state| no_cut(state, 1, "There is no cut 1"),
        does: Does::Act(|_| Some(Action::CutRemove { cut: 0 })),
        ..PLAIN
    },
    Command {
        id: "scene.uncut.2",
        name: "Remove cut 2",
        label: "Remove cut 2",
        explains: "Put back what this cut took away",
        words: UNCUT,
        place: Place::Tab(Tab::Scene),
        why_not: |state| no_cut(state, 2, "There is no cut 2"),
        does: Does::Act(|_| Some(Action::CutRemove { cut: 1 })),
        ..PLAIN
    },
    Command {
        id: "scene.uncut.3",
        name: "Remove cut 3",
        label: "Remove cut 3",
        explains: "Put back what this cut took away",
        words: UNCUT,
        place: Place::Tab(Tab::Scene),
        why_not: |state| no_cut(state, 3, "There is no cut 3"),
        does: Does::Act(|_| Some(Action::CutRemove { cut: 2 })),
        ..PLAIN
    },
    Command {
        id: "scene.slice",
        name: "Draw the slice shown in 2D inside the 3D scene",
        label: "Slice in 3D",
        words: &["slice", "plane", "2D", "inside"],
        place: Place::Tab(Tab::Scene),
        why_not: |state| (look_drawn(state) == Look::Parts).then_some(NO_SLICE_AMONG_PARTS),
        does: Does::Act(|state| Some(Action::ShowSliceInScene(!state.scene.slice_shown))),
        ..PLAIN
    },
    Command {
        // The views menu saves under the name typed beside the button; from anywhere else the
        // view is saved under a name of Voxa's choosing.
        id: "scene.view.save",
        name: "Save this view of the 3D scene",
        label: "Save this view",
        explains: "Keep where the scene is seen from, its look, its window and its cuts",
        words: &["bookmark", "remember", "camera", "views"],
        place: Place::Tab(Tab::Scene),
        why_not: not_ready,
        does: Does::Act(|_| {
            let name = String::new();
            Some(Action::Views(ViewsAction::Save { name }))
        }),
        ..PLAIN
    },
    Command {
        id: "scene.save.picture",
        name: "Save the 3D view as a picture",
        label: "Save picture…",
        explains: "The 3D view as it is now, as a PNG file",
        words: &["png", "screenshot", "image", "export"],
        place: Place::Tab(Tab::Scene),
        why_not: not_ready,
        does: Does::Ask(Request::SaveScene(export::Asked::Picture)),
        ..PLAIN
    },
    Command {
        id: "scene.save.turntable",
        name: "Save a turntable of the 3D view",
        label: "Save turntable…",
        explains: "The 3D view going once round the patient, as an animated GIF",
        words: &["gif", "animation", "spin", "rotate", "export"],
        place: Place::Tab(Tab::Scene),
        why_not: not_ready,
        does: Does::Ask(Request::SaveScene(export::Asked::Turntable)),
        ..PLAIN
    },
];
