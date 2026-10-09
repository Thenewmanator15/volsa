//! The measurements window: clicking down it finds everything it offers.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};
use voxa_core::Point;

use super::*;
use crate::annotations::Shape;
use crate::reducer::reduce;
use crate::testing::thick_scan_ready;

/// A series ready to measure on, with a ruler and a marker measured on it.
fn measured_twice() -> AppState {
    let mut state = thick_scan_ready();
    let ruler = Shape::Ruler {
        a: [0.0, 0.0, 10.0],
        b: [3.0, -4.0, 10.0],
    };
    let marker = Shape::Marker {
        at: [-12.0, 4.5, 30.25],
    };
    for shape in [ruler, marker] {
        let _ = reduce(&mut state, Action::AnnotationAdd(shape));
    }
    state
}

/// The window's contents on a screen of their own, as they are kept between passes.
#[derive(Default)]
struct Shown {
    ctx: egui::Context,
    editing: Option<(u64, String)>,
    /// How far down the contents reached in the last pass.
    bottom: f32,
    /// Everything written in the last pass.
    words: String,
}

/// What one pass asked for.
type Asked = (Vec<Action>, Option<Save>);

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

fn pass(shown: &mut Shown, state: &AppState, events: Vec<Event>) -> Asked {
    let mut actions = Vec::new();
    let mut asked = None;
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(440.0, 400.0));
    let input = RawInput {
        screen_rect: Some(screen),
        events,
        ..RawInput::default()
    };
    let (ctx, editing) = (shown.ctx.clone(), &mut shown.editing);
    let mut bottom = 0.0;
    let mut output = ctx.run_ui(input, |ui| {
        asked = contents(ui, state, editing, &mut actions);
        bottom = ui.min_rect().bottom();
    });
    // No renderer is behind these passes to take the font texture.
    output.textures_delta.clear();
    shown.bottom = bottom;
    shown.words.clear();
    for clipped in &output.shapes {
        words_of(&clipped.shape, &mut shown.words);
    }
    (actions, asked)
}

/// Press and release the main button at a point.
fn click(shown: &mut Shown, state: &AppState, at: Pos2) -> Asked {
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let _ = pass(shown, state, vec![Event::PointerMoved(at), button(true)]);
    pass(shown, state, vec![button(false)])
}

/// Click all over the contents and gather everything that was asked for, each thing once,
/// in the order met going down.
fn sweep(state: &AppState) -> (Vec<Action>, Vec<Save>) {
    let mut shown = Shown::default();
    let _ = pass(&mut shown, state, Vec::new());
    let (mut actions, mut saves) = (Vec::new(), Vec::new());
    let mut y = 3.0;
    while y < shown.bottom {
        for x in (3..410u16).step_by(8) {
            let (asked_for, save) = click(&mut shown, state, pos2(f32::from(x), y));
            for action in asked_for {
                if !actions.contains(&action) {
                    actions.push(action);
                }
            }
            saves.extend(save.filter(|save| !saves.contains(save)));
        }
        y += 6.0;
    }
    (actions, saves)
}

fn position(x: f64, y: f64, z: f64) -> Action {
    Action::SetPosition(Point::new(x, y, z))
}

#[test]
fn clicking_down_the_window_finds_everything_it_offers() {
    let state = measured_twice();
    let ids: Vec<u64> = state.annotations.items.iter().map(|item| item.id).collect();
    let (actions, saves) = sweep(&state);
    for tool in [Tool::Ruler, Tool::Angle, Tool::Region, Tool::Marker] {
        assert!(actions.contains(&Action::SetTool(tool)), "{tool:?}");
    }
    let in_hand = Action::SetTool(Tool::Navigate);
    assert!(
        !actions.contains(&in_hand),
        "the tool in hand is not asked for again"
    );
    assert!(actions.contains(&Action::Undo));
    assert!(
        !actions.contains(&Action::Redo),
        "nothing has been taken back"
    );
    for id in ids {
        assert!(actions.contains(&Action::AnnotationRemove(id)), "{id}");
    }
    assert!(
        actions.contains(&position(0.0, 0.0, 10.0)),
        "the ruler's first end"
    );
    assert!(actions.contains(&position(-12.0, 4.5, 30.25)), "the marker");
    assert!(actions.contains(&Action::AnnotationsClear));
    assert_eq!(saves, [Save::Csv, Save::Json]);
    let renamed = |action: &Action| matches!(action, Action::AnnotationRename { .. });
    assert!(
        !actions.iter().any(renamed),
        "a click in a name changes no name"
    );
}

#[test]
fn what_was_taken_back_can_be_done_again() {
    let mut state = measured_twice();
    let _ = reduce(&mut state, Action::Undo);
    let (actions, _) = sweep(&state);
    assert!(
        actions.contains(&Action::Undo),
        "the ruler can be taken back too"
    );
    assert!(actions.contains(&Action::Redo));
}

#[test]
fn with_nothing_measured_there_is_nothing_to_save_clear_or_take_back() {
    let state = thick_scan_ready();
    let (actions, saves) = sweep(&state);
    let tools = [Tool::Ruler, Tool::Angle, Tool::Region, Tool::Marker].map(Action::SetTool);
    assert_eq!(actions, tools, "the tools, and nothing else");
    assert_eq!(saves, []);
    let mut shown = Shown::default();
    let _ = pass(&mut shown, &state, Vec::new());
    assert!(
        shown.words.contains("Nothing has been measured"),
        "{}",
        shown.words
    );
    assert!(!shown.words.contains(NO_VOLUME));
}

#[test]
fn with_no_volume_the_window_says_so_and_offers_no_tool() {
    let state = AppState::default();
    let (actions, saves) = sweep(&state);
    assert_eq!(actions, []);
    assert_eq!(saves, []);
    let mut shown = Shown::default();
    let _ = pass(&mut shown, &state, Vec::new());
    assert!(shown.words.contains(NO_VOLUME), "{}", shown.words);
    assert_eq!(NO_VOLUME, "Measuring needs a 3D view of the series.");
}

#[test]
fn each_row_says_what_was_measured_and_how_exactly() {
    let state = measured_twice();
    let mut shown = Shown::default();
    let _ = pass(&mut shown, &state, Vec::new());
    let placement = grid(&state).map(|(placement, _)| placement);
    for item in &state.annotations.items {
        let said = measured(&item.shape, placement.as_ref());
        assert!(
            shown.words.contains(&said.value),
            "{} in {}",
            said.value,
            shown.words
        );
        assert!(shown.words.contains(&said.detail), "{}", said.detail);
    }
    assert!(shown.words.contains("5.0 mm"), "{}", shown.words);
}

#[test]
fn a_name_is_taken_when_the_person_leaves_the_field() {
    let state = measured_twice();
    let first = state.annotations.items[0].clone();
    let mut shown = Shown::default();
    let _ = pass(&mut shown, &state, Vec::new());
    // Click down the left of the window until a name takes the keys.
    let mut y = 3.0;
    while shown.editing.is_none() && y < shown.bottom {
        let _ = click(&mut shown, &state, pos2(30.0, y));
        y += 4.0;
    }
    let (id, typed) = shown.editing.clone().expect("a name took the keys");
    assert_eq!((id, typed), (first.id, first.label.clone()));
    let (asked, _) = pass(&mut shown, &state, vec![Event::Text("abc".to_owned())]);
    assert_eq!(asked, [], "typing alone changes no name");
    let (_, typed) = shown.editing.clone().expect("still being typed");
    // Clicks one after another count as a double-click, which selects what is typed over.
    assert!(typed.contains("abc") && typed != first.label, "{typed}");
    // A click on nothing, well below the window's contents, leaves the field.
    let (asked, _) = click(&mut shown, &state, pos2(200.0, 380.0));
    let label = typed;
    assert_eq!(asked, [Action::AnnotationRename { id, label }]);
    assert_eq!(shown.editing, None);
}

#[test]
fn the_window_is_there_only_while_it_is_open() {
    let state = measured_twice();
    let words = |open: bool| {
        let ctx = egui::Context::default();
        let mut panel = Panel {
            open,
            ..Panel::default()
        };
        let mut said = String::new();
        // A window is laid out on its first pass and drawn from its second.
        for _ in 0..2 {
            let mut actions = Vec::new();
            let mut output = ctx.run_ui(RawInput::default(), |ui| {
                assert_eq!(panel.show(ui.ctx(), &state, &mut actions), None);
            });
            output.textures_delta.clear();
            said.clear();
            for clipped in &output.shapes {
                words_of(&clipped.shape, &mut said);
            }
            assert_eq!(actions, []);
        }
        (said, panel.open)
    };
    let (said, open) = words(true);
    assert!(
        open && said.contains("Measurements") && said.contains("Save as CSV"),
        "{said}"
    );
    let (said, open) = words(false);
    assert!(!open && !said.contains("Measurements"), "{said}");
}
