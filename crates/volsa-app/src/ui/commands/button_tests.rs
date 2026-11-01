//! An action's button: it shows its label, a screen reader hears its name, and it can be
//! pressed only while the action can be used.

use egui::{Button, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};

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
    // The name does not hold the label, so the label comes first.
    assert!(
        drawn.names.contains(&"4, Four viewports".to_owned()),
        "{:?}",
        drawn.names
    );
    let four = Some(Action::SetLayout(Layout::TwoByTwo));
    assert_eq!(acted(given, &state), four);
}

/// The command `id`, which the table lists.
fn listed(id: &str) -> &'static Command {
    find(id).expect("listed")
}

#[test]
fn a_name_that_holds_the_label_is_the_name_alone() {
    // Whatever the case: a person who says the words they see is understood.
    for (id, label) in [
        ("pictures.slices", "Slices"),
        ("pictures.scene", "3D"),
        ("planes.link", "Link"),
        ("tab.measure", "MEASURE"),
        ("search.open", "search"),
    ] {
        let command = listed(id);
        assert_eq!(spoken(command, label), command.name, "{id} says {label}");
    }
}

#[test]
fn a_name_that_lacks_the_label_says_the_label_first() {
    let said = |id: &str, label: &str| spoken(listed(id), label).into_owned();
    assert_eq!(said("layout.one", "1"), "1, One viewport");
    assert_eq!(
        said("scene.look.vr", "VR"),
        "VR, Show the scan by volume rendering"
    );
    assert_eq!(
        said("view.radiological", "Radiological"),
        "Radiological, Patient's left on the right"
    );
    assert_eq!(said("measure.ruler", "Ruler"), "Ruler, Measure a length");
    // The search box writes its own words, not the action's label.
    assert_eq!(
        said("search.open", "Find a tool or part"),
        "Find a tool or part, Search actions, parts, scans and help"
    );
}

#[test]
fn a_mark_with_no_letter_or_digit_is_a_picture_and_the_name_stands_alone() {
    for (id, label) in [
        ("volume.before", "◀"),
        ("volume.after", "▶"),
        ("panel.fold", "«"),
        ("help.keys", "?"),
        ("theme.light", ""),
    ] {
        let command = listed(id);
        assert_eq!(spoken(command, label), command.name, "{id} shows {label:?}");
    }
}

#[test]
fn a_button_made_for_the_action_is_named_by_the_words_it_writes() {
    let state = &AppState::default();
    let draw = |shows: &'static str| {
        move |ui: &mut egui::Ui| {
            let made = button_made(ui, "search.open", state, shows, |_| Button::new(shows));
            made.and_then(|(_, does)| does)
        }
    };
    let (drawn, _) = pressed(draw("Find a tool or part"));
    let said = "Find a tool or part, Search actions, parts, scans and help".to_owned();
    assert!(drawn.names.contains(&said), "{:?}", drawn.names);
    // A picture alone is named by the name.
    let (drawn, _) = pressed(draw(""));
    let name = "Search actions, parts, scans and help".to_owned();
    assert!(drawn.names.contains(&name), "{:?}", drawn.names);
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
    // The name holds "pause" in any case, so it stands alone.
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
