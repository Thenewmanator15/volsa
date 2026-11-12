//! Made-up labels, and a screen of a panel's width to draw the Labels and Tools tabs on, for
//! their tests and those of the strip the Tools tab holds.

use std::path::PathBuf;

use egui::Ui;
use volsa_labels::{Made, Part, Side, Trust, Verdict};
use volsa_models::{Card, CardId, CardKind};

use super::{labels, tools};
use crate::action::Action;
use crate::labelling::Ready;
use crate::labels::NamesFrom;
use crate::reducer::reduce;
use crate::state::AppState;
use crate::testing::thick_scan_ready;
use crate::ui::commands::Request;
use crate::ui::hand::Hand;
use crate::ui::parts::PartsView;

/// How wide the screen is, in points: a panel dragged wide enough for a row to have room for
/// the volume of each side of a part.
pub const WIDE: f32 = 360.0;

/// A made-up part.
pub fn part(name: &str, group: &str, side: Side, trust: Trust) -> Part {
    Part {
        name: name.to_owned(),
        group: group.to_owned(),
        side,
        colour: [200, 120, 40],
        trust,
        voxels: 4000,
        ml: 4.0,
    }
}

/// Four made-up parts in three groups: a pair of sides, a part on the midline and one that the
/// rules made.
pub fn parts() -> Vec<Part> {
    let rules = Trust::Approximate("Rules.".to_owned());
    vec![
        part("hippocampus", "Deep brain", Side::Left, Trust::Model),
        part("hippocampus", "Deep brain", Side::Right, Trust::Model),
        part(
            "brain stem",
            "Cerebellum and brainstem",
            Side::Midline,
            Trust::Model,
        ),
        part("eyeball", "Outside the head's brain", Side::Left, rules),
    ]
}

/// A series with its volume ready and labels of it read, made by `made` if anyone.
pub fn labelled_by(
    verdict: Option<Verdict>,
    names_from: NamesFrom,
    made: Option<Made>,
) -> AppState {
    labelled_with(parts(), verdict, names_from, made)
}

/// As [`labelled_by`], with these parts.
pub fn labelled_with(
    parts: Vec<Part>,
    verdict: Option<Verdict>,
    names_from: NamesFrom,
    made: Option<Made>,
) -> AppState {
    let mut state = thick_scan_ready();
    let open = Action::OpenLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    };
    reduce(&mut state, open);
    let read = Action::LabelsRead {
        title: "A made-up head".to_owned(),
        parts,
        verdict,
        corners: state.scene.corners.expect("the volume's box"),
        names_from,
        made,
        run: None,
    };
    reduce(&mut state, read);
    state
}

/// A series with its volume ready and labels of it read, with a table of names.
pub fn named() -> AppState {
    labelled_by(None, NamesFrom::Table, None)
}

/// What a tab draws: it is given the state, the actions to add changes of state to, and gives
/// what it asked of the window.
pub type Draw<'a> = dyn FnMut(&mut Ui, &AppState, &mut Vec<Action>) -> Vec<Request> + 'a;

/// The screen the tabs are drawn on.
pub fn screen() -> Hand {
    Hand::new(WIDE, 900.0)
}

/// Each thing once, in the order first met.
pub fn once<T: PartialEq>(all: Vec<T>) -> Vec<T> {
    let mut found: Vec<T> = Vec::new();
    for one in all {
        if !found.contains(&one) {
            found.push(one);
        }
    }
    found
}

/// Click on every button of what `draw` draws for `state`, once each, and gather everything
/// that was asked for, each thing once: the changes of state, and what was asked of the window.
pub fn sweep(state: &AppState, draw: &mut Draw<'_>) -> (Vec<Action>, Vec<Request>) {
    let (mut actions, mut requests) = (Vec::new(), Vec::new());
    screen().click_each(&mut |ui| requests.extend(draw(ui, state, &mut actions)));
    (once(actions), once(requests))
}

/// Everything `draw` writes for `state`, each piece of text on a line of its own, in the order
/// it was painted.
pub fn written(state: &AppState, draw: &mut Draw<'_>) -> String {
    let seen = screen().settle(&mut |ui| {
        let _ = draw(ui, state, &mut Vec::new());
    });
    seen.words.join("\n")
}

/// The Labels tab, drawn with a search box of its own.
pub fn labels_tab() -> impl FnMut(&mut Ui, &AppState, &mut Vec<Action>) -> Vec<Request> {
    let mut view = PartsView::default();
    move |ui, state, actions| labels::show(ui, state, &mut view, actions)
}

/// The Tools tab, with no tool added by the person.
pub fn tools_tab() -> impl FnMut(&mut Ui, &AppState, &mut Vec<Action>) -> Vec<Request> {
    |ui, state, actions| tools::show(ui, state, actions)
}

/// Click on every button of the Labels tab.
pub fn sweep_labels(state: &AppState) -> (Vec<Action>, Vec<Request>) {
    sweep(state, &mut labels_tab())
}

/// What the Labels tab writes.
pub fn labels_written(state: &AppState) -> String {
    written(state, &mut labels_tab())
}

/// Click on every button of the Tools tab.
pub fn sweep_tools(state: &AppState) -> (Vec<Action>, Vec<Request>) {
    sweep(state, &mut tools_tab())
}

/// What the Tools tab writes.
pub fn tools_written(state: &AppState) -> String {
    written(state, &mut tools_tab())
}

/// A made-up graphics card.
pub fn card(number: u64, name: &str, kind: CardKind) -> Card {
    Card {
        id: CardId(number),
        index: u32::try_from(number).expect("a small number"),
        name: name.to_owned(),
        kind,
        own_bytes: 8 << 30,
    }
}

/// An integrated card, a discrete one and a software one.
pub fn cards() -> Vec<Card> {
    vec![
        card(1, "Made-up shared graphics", CardKind::Integrated),
        card(2, "Made-up card A", CardKind::Discrete),
        card(3, "Made-up software renderer", CardKind::Software),
    ]
}

/// The 30-slice series with its volume built, on a computer that can label, with no labels.
pub fn able() -> AppState {
    let mut state = thick_scan_ready();
    state.labelling.ready = Ready::Yes;
    state.labelling.cards = cards();
    state
}
