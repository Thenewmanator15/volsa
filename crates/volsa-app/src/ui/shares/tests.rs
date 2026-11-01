//! How wide the series list is, and the line between two parts of the window that can be
//! dragged.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Sense, pos2, vec2};

use super::*;

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn the_series_list_takes_a_share_of_the_window_within_limits() {
    let share = 1.0 / 6.0;
    assert!(near(list_width(1440.0, share), 240.0));
    assert!(near(list_width(1920.0, share), 320.0));
    assert_eq!(
        list_width(3840.0, share),
        360.0,
        "no wider than its cards can use"
    );
    assert_eq!(
        list_width(960.0, share),
        220.0,
        "no narrower than a card can be read"
    );
    assert!(
        near(list_width(400.0, share), 140.0),
        "but never more than 35% of the window"
    );
}

#[test]
fn dragging_the_series_list_changes_its_share_as_far_as_the_limits_allow() {
    let share = 1.0 / 6.0;
    let wider = list_dragged(1440.0, share, 60.0);
    assert!(near(wider * 1440.0, 300.0), "{wider}");
    let narrower = list_dragged(1440.0, share, -15.0);
    assert!(near(narrower * 1440.0, 225.0), "{narrower}");
    assert!(near(list_dragged(1440.0, share, 1000.0) * 1440.0, 360.0));
    assert!(near(list_dragged(1440.0, share, -1000.0) * 1440.0, 220.0));
    // What is kept is the share, so that a larger window gives the list as much more.
    assert!(
        near(list_width(1600.0, wider), 333.33),
        "{}",
        list_width(1600.0, wider)
    );
}

/// Draw a widget that takes drags over the whole screen, with a divider at `x` above it,
/// for each list of events in turn, and add up how far the divider says it was dragged.
fn dragged_by(x: f32, steps: Vec<Vec<Event>>) -> f32 {
    let ctx = egui::Context::default();
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    let mut moved = 0.0;
    for events in std::iter::once(Vec::new()).chain(steps) {
        let input = RawInput {
            screen_rect: Some(screen),
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let _ = ui.interact(screen, egui::Id::new("under"), Sense::drag());
            moved += divider(ui, egui::Id::new("edge"), x, screen.y_range(), "Edge");
        });
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
    }
    moved
}

/// Press the main button at `from`, move across by `by` in a few steps, and let go.
fn drag(from: Pos2, by: f32) -> Vec<Vec<Event>> {
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let to = from + vec2(by, 0.0);
    let mut steps = vec![vec![Event::PointerMoved(from), button(from, true)]];
    for step in 1..=4u8 {
        let at = from + vec2(by * f32::from(step) / 4.0, 0.0);
        steps.push(vec![Event::PointerMoved(at)]);
    }
    steps.push(vec![button(to, false)]);
    steps
}

#[test]
fn the_divider_says_how_far_it_was_dragged_and_no_widget_beneath_takes_the_drag() {
    let rightwards = dragged_by(200.0, drag(pos2(201.0, 300.0), 40.0));
    assert!(rightwards > 0.0 && rightwards <= 40.0, "{rightwards}");
    let leftwards = dragged_by(200.0, drag(pos2(199.0, 300.0), -40.0));
    assert!((-40.0..0.0).contains(&leftwards), "{leftwards}");
    let elsewhere = dragged_by(200.0, drag(pos2(400.0, 300.0), 40.0));
    assert_eq!(
        elsewhere, 0.0,
        "a drag away from the line is not the line's"
    );
}

#[test]
fn a_divider_is_called_what_it_resizes_for_a_screen_reader() {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    let input = RawInput {
        screen_rect: Some(screen),
        ..RawInput::default()
    };
    let name = "Width of the scans";
    let mut output = ctx.run_ui(input, |ui| {
        let _ = divider(ui, egui::Id::new("edge"), 200.0, screen.y_range(), name);
    });
    // No renderer is behind this pass to take the font texture.
    output.textures_delta.clear();
    let update = output.platform_output.accesskit_update;
    let update = update.expect("the interface is described");
    let found = update
        .nodes
        .iter()
        .find(|(_, node)| node.label() == Some(name));
    let (_, node) = found.expect("the divider is named");
    assert_eq!(node.role(), egui::accesskit::Role::Splitter);
}
