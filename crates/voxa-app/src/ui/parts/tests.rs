//! The Parts list: what it lists for a search, and that clicking down it finds everything it
//! offers.

use std::path::PathBuf;

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};
use voxa_labels::{Made, Part, Verdict};

use super::*;
use crate::reducer::reduce;
use crate::testing::thick_scan_ready;

pub(super) fn part(name: &str, group: &str, side: Side, trust: Trust) -> Part {
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

pub(super) fn parts() -> Vec<Part> {
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

/// A series with its volume ready and labels of it read.
fn labelled(verdict: Option<Verdict>, names_from: NamesFrom) -> AppState {
    labelled_by(verdict, names_from, None)
}

/// As [`labelled`], with the labels made by `made`, if anyone.
pub(super) fn labelled_by(
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
        parts: parts(),
        verdict,
        corners: state.scene.corners.expect("the volume's box"),
        names_from,
        made,
        run: None,
    };
    reduce(&mut state, read);
    state
}

pub(super) fn named() -> AppState {
    labelled(None, NamesFrom::Table)
}

/// The list on a screen of its own, as it is kept between passes.
#[derive(Default)]
pub(super) struct Shown {
    ctx: egui::Context,
    view: PartsView,
    pub(super) words: String,
    /// How many bytes of the graphics card the window holds, as the strip is told.
    pub(super) held: u64,
    /// Whether the strip's menu is held open for every pass.
    pub(super) menu: bool,
}

impl Shown {
    /// A screen told that the window holds `held` bytes of its graphics card, with the
    /// strip's menu held open if `menu`.
    pub(super) fn with(held: u64, menu: bool) -> Self {
        Self {
            held,
            menu,
            ..Self::default()
        }
    }
}

fn words_of(shape: &egui::Shape, into: &mut String) {
    match shape {
        egui::Shape::Text(text) => {
            into.push_str(text.galley.text());
            into.push('\n');
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                words_of(shape, into);
            }
        }
        _ => {}
    }
}

pub(super) fn pass(
    shown: &mut Shown,
    state: &AppState,
    events: Vec<Event>,
) -> (Vec<Action>, Option<Asked>) {
    let (mut actions, mut asked) = (Vec::new(), None);
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(360.0, 420.0));
    let input = RawInput {
        screen_rect: Some(screen),
        events,
        ..RawInput::default()
    };
    if shown.menu {
        egui::Popup::open_id(&shown.ctx, labelling::menu_id());
    }
    let (ctx, view, held) = (shown.ctx.clone(), &mut shown.view, shown.held);
    let mut output = ctx.run_ui(input, |ui| {
        asked = show(ui, screen, state, held, view, &mut actions);
    });
    // No renderer is behind these passes to take the font texture.
    output.textures_delta.clear();
    shown.words.clear();
    for clipped in &output.shapes {
        words_of(&clipped.shape, &mut shown.words);
    }
    (actions, asked)
}

/// Click all over the list and gather everything that was asked for, each thing once.
fn sweep(state: &AppState) -> (Vec<Action>, Vec<Asked>) {
    sweep_with(state, Shown::default())
}

/// As [`sweep`], on a screen set up as `shown` says.
pub(super) fn sweep_with(state: &AppState, mut shown: Shown) -> (Vec<Action>, Vec<Asked>) {
    let _ = pass(&mut shown, state, Vec::new());
    let (mut actions, mut asked) = (Vec::new(), Vec::new());
    for y in (3..420_u16).step_by(6) {
        for x in (3..360_u16).step_by(10) {
            let at = pos2(f32::from(x), f32::from(y));
            let button = |pressed| Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            let _ = pass(
                &mut shown,
                state,
                vec![Event::PointerMoved(at), button(true)],
            );
            let (asked_for, wanted) = pass(&mut shown, state, vec![button(false)]);
            for action in asked_for {
                if !actions.contains(&action) {
                    actions.push(action);
                }
            }
            asked.extend(wanted.filter(|wanted| !asked.contains(wanted)));
        }
    }
    (actions, asked)
}

pub(super) fn written(state: &AppState) -> String {
    let mut shown = Shown::default();
    let _ = pass(&mut shown, state, Vec::new());
    shown.words
}

#[test]
fn the_list_is_the_tables_groups_with_a_name_of_two_sides_as_one_row() {
    let state = named();
    let listed = rows(&state.labels, "");
    let said: Vec<(&str, Vec<&str>)> = listed
        .iter()
        .map(|group| {
            (
                group.name.as_str(),
                group.rows.iter().map(|row| row.name.as_str()).collect(),
            )
        })
        .collect();
    let expected = [
        ("Deep brain", vec!["hippocampus"]),
        ("Cerebellum and brainstem", vec!["brain stem"]),
        ("Outside the head's brain", vec!["eyeball"]),
    ];
    assert_eq!(said, expected);
    let both = &listed[0].rows[0];
    assert_eq!(
        (both.left, both.right, both.midline),
        (Some(0), Some(1), None)
    );
}

#[test]
fn a_search_keeps_the_rows_with_those_letters_in_their_name_or_group() {
    let state = named();
    let found = |search: &str| -> Vec<String> {
        let groups = rows(&state.labels, search);
        groups
            .iter()
            .flat_map(|group| &group.rows)
            .map(|row| row.name.clone())
            .collect()
    };
    assert_eq!(
        found("HIP"),
        ["hippocampus"],
        "whatever the case of the letters"
    );
    assert_eq!(found("  stem "), ["brain stem"]);
    assert_eq!(found("deep"), ["hippocampus"], "by its group");
    assert_eq!(found("zzz"), Vec::<String>::new());
    assert_eq!(
        rows(&state.labels, "hip").len(),
        1,
        "and only its group is left"
    );
}

#[test]
fn clicking_down_the_list_finds_everything_it_offers() {
    let state = named();
    let (actions, asked) = sweep(&state);
    for part in 0..4 {
        let hide = Action::PartShow { part, shown: false };
        assert!(
            actions.contains(&hide),
            "a switch for part {part}: {actions:?}"
        );
    }
    for part in [0, 2, 3] {
        assert!(
            actions.contains(&Action::PartSelect(Some(part))),
            "a name selects part {part}"
        );
    }
    for group in [
        "Deep brain",
        "Cerebellum and brainstem",
        "Outside the head's brain",
    ] {
        let hide = Action::GroupShow {
            group: group.to_owned(),
            shown: false,
        };
        assert!(actions.contains(&hide), "{group}");
    }
    assert!(actions.contains(&Action::PartsShowAll) && actions.contains(&Action::CloseLabels));
    assert!(actions.contains(&Action::SetOverlay(Overlay::Off)));
    assert!(actions.contains(&Action::SetOverlay(Overlay::Filled)));
    assert!(
        !actions.contains(&Action::SetOverlay(Overlay::Outline)),
        "the one in use"
    );
    assert_eq!(asked, [Asked::OpenNames, Asked::SaveCsv]);
}

#[test]
fn the_part_selected_can_be_faded_shown_alone_and_let_go() {
    let mut state = named();
    reduce(&mut state, Action::PartSelect(Some(1)));
    let (actions, _) = sweep(&state);
    assert!(actions.contains(&Action::PartIsolate(1)), "{actions:?}");
    let faded = |action: &Action| matches!(action, Action::PartOpacity { part: 1, .. });
    assert!(actions.iter().any(faded), "{actions:?}");
    assert!(
        actions.contains(&Action::PartSelect(None)),
        "its own name lets it go"
    );
    let said = written(&state);
    assert!(said.contains("hippocampus, right: 4.0 ml"), "{said}");
    let against = "Left 4.0 ml, right 4.0 ml: the same size.";
    assert!(said.contains(against), "its two sides are compared: {said}");
    reduce(&mut state, Action::PartSelect(Some(2)));
    let said = written(&state);
    assert!(
        !said.contains("Left "),
        "a part with one side is not: {said}"
    );
}

#[test]
fn the_list_says_what_the_labels_are_where_their_names_came_from_and_the_models_check() {
    let said = written(&named());
    for sentence in ["A made-up head", MADE_ELSEWHERE, "Names from the table"] {
        assert!(said.contains(sentence), "{sentence}: {said}");
    }
    assert!(
        said.contains("approximate"),
        "how far a name can be trusted: {said}"
    );
    let verdict = Verdict {
        passed: false,
        fail_below: 0.65,
        low: vec![("general csf".to_owned(), 0.6)],
    };
    let said = written(&labelled(Some(verdict), NamesFrom::Nothing));
    assert!(
        said.contains("The labelling model's own check failed"),
        "{said}"
    );
    assert!(said.contains("called by their numbers"), "{said}");
}

#[test]
fn with_no_labels_the_tab_says_what_they_are_and_offers_to_open_some() {
    let state = thick_scan_ready();
    let (actions, asked) = sweep(&state);
    assert_eq!((actions, asked), (vec![], vec![Asked::OpenLabels]));
    assert!(
        written(&state).contains("with a table of names beside it if it has one. Voxa shows them.")
    );
    // With no volume there is nothing to put labels on.
    let (actions, asked) = sweep(&AppState::default());
    assert_eq!((actions, asked), (vec![], vec![]));
}

#[test]
fn while_labels_are_read_and_when_they_fail_the_tab_says_so() {
    let mut state = thick_scan_ready();
    let open = Action::OpenLabels {
        labels: PathBuf::from("labels.nii.gz"),
        table: None,
    };
    reduce(&mut state, open);
    assert!(written(&state).contains("Reading the labels"));
    reduce(
        &mut state,
        Action::LabelsFailed("This is not a NIfTI file.".to_owned()),
    );
    assert!(written(&state).contains("This is not a NIfTI file."));
    let (_, asked) = sweep(&state);
    assert_eq!(asked, [Asked::OpenLabels], "and another can be tried");
}

#[test]
fn labels_of_another_study_are_said_to_be_and_can_be_closed() {
    let mut state = named();
    state.labels.study = Some(voxa_core::StudyId("another".to_owned()));
    assert!(written(&state).contains("another study"));
    let (actions, asked) = sweep(&state);
    assert_eq!((actions, asked), (vec![Action::CloseLabels], vec![]));
}
