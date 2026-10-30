//! An action's button: it shows its label, a screen reader hears its name, and it can be
//! pressed only while the action can be used.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};

use super::*;
use crate::annotations::Tool;
use crate::state::Layout;
use crate::testing::thick_scan_ready;

/// What one pass of a button drew: the words, and the names given to a screen reader.
#[derive(Default)]
struct Drawn {
    words: Vec<String>,
    names: Vec<String>,
}

/// The words written by a shape and the shapes within it.
fn words_of(shape: &egui::Shape, into: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(text) => into.push(text.galley.text().to_owned()),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                words_of(shape, into);
            }
        }
        _ => {}
    }
}

/// One pass of `draw` in a small window, with these events: what it gave, and what it drew.
fn pass(
    ctx: &egui::Context,
    events: Vec<Event>,
    draw: &impl Fn(&mut egui::Ui) -> Option<Does>,
) -> (Option<Does>, Drawn) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 200.0))),
        events,
        ..RawInput::default()
    };
    let mut given = None;
    let mut output = ctx.run_ui(input, |ui| given = draw(ui));
    // No renderer is behind these passes to take the font texture.
    output.textures_delta.clear();
    let mut drawn = Drawn::default();
    for clipped in &output.shapes {
        words_of(&clipped.shape, &mut drawn.words);
    }
    let update = output.platform_output.accesskit_update.into_iter();
    let nodes = update.flat_map(|update| update.nodes);
    drawn.names = nodes
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect();
    (given, drawn)
}

/// Draw a button and click along the line it is on: what it drew, and what the first click
/// that met it gave.
fn pressed(draw: impl Fn(&mut egui::Ui) -> Option<Does>) -> (Drawn, Option<Does>) {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let (_, drawn) = pass(&ctx, Vec::new(), &draw);
    for x in (2..120u8).step_by(4) {
        let at = pos2(f32::from(x), 12.0);
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let _ = pass(&ctx, vec![Event::PointerMoved(at), button(true)], &draw);
        let (given, _) = pass(&ctx, vec![button(false)], &draw);
        if given.is_some() {
            return (drawn, given);
        }
    }
    (drawn, None)
}

/// The change of state a click gave, if it gave one.
fn acted(given: Option<Does>, state: &AppState) -> Option<Action> {
    match given? {
        Does::Act(act) => act(state),
        Does::Ask(_) => None,
    }
}

#[test]
fn a_button_shows_its_label_and_is_named_by_its_name() {
    let state = AppState::default();
    let (drawn, given) = pressed(|ui| button(ui, "layout.four", &state, Some(false)));
    assert!(drawn.words.contains(&"4".to_owned()), "{:?}", drawn.words);
    assert!(
        drawn.names.contains(&"Four viewports".to_owned()),
        "{:?}",
        drawn.names
    );
    let four = Some(Action::SetLayout(Layout::TwoByTwo));
    assert_eq!(acted(given, &state), four);
}

#[test]
fn a_label_that_follows_the_state_keeps_the_name() {
    let state = thick_scan_ready();
    let draw = |ui: &mut egui::Ui| button_labelled(ui, "slice.play", "Pause", &state, None);
    let (drawn, given) = pressed(draw);
    assert!(
        drawn.words.contains(&"Pause".to_owned()),
        "{:?}",
        drawn.words
    );
    let name = "Play or pause through the slices".to_owned();
    assert!(drawn.names.contains(&name), "{:?}", drawn.names);
    let toggled = Some(Action::CineToggle { viewport: 0 });
    assert_eq!(acted(given, &state), toggled);
}

#[test]
fn a_button_cannot_be_pressed_while_its_action_cannot_be_used() {
    let nothing = AppState::default();
    let (drawn, given) = pressed(|ui| button(ui, "measure.ruler", &nothing, None));
    assert!(
        drawn.words.contains(&"Ruler".to_owned()),
        "greyed, not hidden"
    );
    assert!(given.is_none(), "no volume to measure on");
    let ready = thick_scan_ready();
    let (_, given) = pressed(|ui| button(ui, "measure.ruler", &ready, None));
    assert_eq!(acted(given, &ready), Some(Action::SetTool(Tool::Ruler)));
}

#[test]
fn a_button_asks_the_window_for_what_is_not_a_change_of_state() {
    let state = AppState::default();
    let (_, given) = pressed(|ui| button(ui, "help.keys", &state, None));
    assert!(
        matches!(given, Some(Does::Ask(Request::Help(None)))),
        "{given:?}"
    );
    let (drawn, given) = pressed(|ui| button(ui, "no.such.action", &state, None));
    assert!(
        given.is_none() && drawn.words.is_empty(),
        "nothing is drawn"
    );
}
