//! The bar across the top of the window.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};

use super::*;

/// How wide the window is for these passes: wide enough for the bar on one line.
const WIDE: u16 = 2000;

fn pass(ctx: &egui::Context, state: &AppState, events: Vec<Event>) -> (Vec<Action>, Asked) {
    let mut actions = Vec::new();
    let mut asked = Asked::default();
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(f32::from(WIDE), 600.0));
    let input = RawInput {
        screen_rect: Some(screen),
        events,
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, |ui| asked = show(ui, state, true, &mut actions));
    // No renderer is behind these passes to take the font texture.
    output.textures_delta.clear();
    (actions, asked)
}

/// Press and release the main button at a point.
fn click(ctx: &egui::Context, state: &AppState, at: Pos2) -> (Vec<Action>, Asked) {
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let _ = pass(ctx, state, vec![Event::PointerMoved(at), button(true)]);
    pass(ctx, state, vec![button(false)])
}

/// Click all the way along the bar and gather everything that was asked for.
fn sweep(state: &AppState) -> (Vec<Action>, Asked) {
    let (actions, all, _) = sweep_with_picks(state);
    (actions, all)
}

/// As [`sweep`], with every kind of thing asked to be opened, in the order met.
fn sweep_with_picks(state: &AppState) -> (Vec<Action>, Asked, Vec<Pick>) {
    let ctx = egui::Context::default();
    let _ = pass(&ctx, state, Vec::new());
    let (mut actions, mut all, mut picks) = (Vec::new(), Asked::default(), Vec::new());
    for x in (3..WIDE).step_by(5) {
        let (asked_for, asked) = click(&ctx, state, pos2(f32::from(x), 12.0));
        for action in asked_for {
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
        picks.extend(asked.open.filter(|pick| !picks.contains(pick)));
        all.open = all.open.or(asked.open);
        all.about |= asked.about;
        all.shortcuts |= asked.shortcuts;
        all.tags |= asked.tags;
        all.measure |= asked.measure;
        all.list |= asked.list;
        all.size = all.size.or(asked.size);
    }
    (actions, all, picks)
}

#[test]
fn the_bar_offers_each_thing_once_from_left_to_right() {
    let (actions, asked, picks) = sweep_with_picks(&AppState::default());
    assert_eq!(picks, [Pick::Folder, Pick::File]);
    let preset = |preset| Action::ApplyPreset {
        viewport: 0,
        preset,
    };
    let expected = vec![
        Action::SetLayout(Layout::TwoAcross),
        Action::SetLayout(Layout::TwoByTwo),
        preset(0),
        preset(1),
        preset(2),
        Action::ToggleRadiological,
        Action::TogglePatient,
        Action::SetLinked(false),
        Action::SetSmooth(true),
        Action::CineToggle { viewport: 0 },
        Action::CineSet {
            viewport: 0,
            fps: Cine::DEFAULT_FPS,
            looping: false,
        },
    ];
    assert_eq!(
        actions, expected,
        "the layout in use is not asked for again"
    );
    assert_eq!(
        asked,
        Asked {
            open: Some(Pick::Folder),
            about: true,
            shortcuts: true,
            tags: true,
            measure: true,
            list: true,
            size: None,
        },
        "opening the Size menu chooses no size"
    );
}

#[test]
fn the_link_says_what_it_takes_for_granted() {
    let taken = "Linked views take it that the patient did not move between series.";
    assert!(LINK_SAYS.ends_with(taken), "{LINK_SAYS}");
    let mut state = AppState::default();
    state.planes.linked = false;
    let (actions, _) = sweep(&state);
    assert!(actions.contains(&Action::SetLinked(true)), "{actions:?}");
}

#[test]
fn the_smooth_switch_says_that_it_adds_no_detail() {
    let honest = "Smoothed: a curve through the measured values, not more detail.";
    assert!(SMOOTH_SAYS.ends_with(honest), "{SMOOTH_SAYS}");
    let mut state = AppState::default();
    state.planes.smooth = true;
    let (actions, _) = sweep(&state);
    assert!(actions.contains(&Action::SetSmooth(false)), "{actions:?}");
}

#[test]
fn the_window_controls_act_on_the_active_viewport() {
    let state = AppState {
        layout: Layout::TwoByTwo,
        active: 2,
        ..AppState::default()
    };
    let (actions, _) = sweep(&state);
    let layouts = [Layout::One, Layout::TwoAcross].map(Action::SetLayout);
    assert!(
        layouts.iter().all(|layout| actions.contains(layout)),
        "{actions:?}"
    );
    assert!(!actions.contains(&Action::SetLayout(Layout::TwoByTwo)));
    for preset in 0..3 {
        assert!(
            actions.contains(&Action::ApplyPreset {
                viewport: 2,
                preset
            }),
            "{actions:?}"
        );
    }
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::SetWindow { .. })),
        "a click changes no number"
    );
}

#[test]
fn in_the_smallest_window_the_bars_wrap_onto_lines_and_stand_no_words_on_end() {
    let ctx = egui::Context::default();
    super::super::theme::apply(&ctx);
    let state = AppState::default();
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(640.0, 480.0));
    let mut high = 0.0;
    for _ in 0..2 {
        let input = RawInput {
            screen_rect: Some(screen),
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let bars = egui::Panel::top("bars").show(ui, |ui| {
                let mut actions = Vec::new();
                let _ = show(ui, &state, false, &mut actions);
                let _ = super::super::scene_bar::show(ui, &state, &mut actions);
            });
            high = bars.response.rect.height();
        });
        // No renderer is behind these passes to take the font texture.
        output.textures_delta.clear();
    }
    assert!(high < 140.0, "the bars take {high} of 480 points");
}
