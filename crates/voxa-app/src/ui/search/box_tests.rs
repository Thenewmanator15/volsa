//! The box itself: what it draws, how it takes the keyboard, and what its keys and clicks do.

use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, vec2};

use super::entries::{Heading, Target, entries_for};
use super::matching::ranked;
use super::run::run;
use super::*;
use crate::action::Action;
use crate::state::{Notice, NoticeKind};
use crate::testing::{labelled_head, thick_scan_ready};
use crate::ui::keys;
use crate::ui::measure::NO_VOLUME;
use crate::ui::theme;

/// One pass of the window with `events`: what search chose and every piece of text it drew,
/// with where.
pub(super) struct Pass {
    pub(super) chosen: Option<Target>,
    pub(super) words: Vec<String>,
    pub(super) places: Vec<(String, Rect)>,
    /// The part of the window each piece of text could be seen in, in the same order.
    pub(super) clips: Vec<Rect>,
}

/// The words written by a shape and the shapes within it, and where each is.
fn words_of(shape: &egui::Shape, clip: Rect, into: &mut Vec<(String, Rect, Rect)>) {
    match shape {
        egui::Shape::Text(text) => {
            let place = text.visual_bounding_rect();
            into.push((text.galley.text().to_owned(), place, clip));
        }
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| words_of(shape, clip, into)),
        _ => {}
    }
}

/// A window drawn with Voxa's look.
pub(super) fn window() -> Context {
    let ctx = Context::default();
    theme::apply(&ctx);
    ctx
}

/// Draw one pass in a window 900 points wide and `height` high in which `events` happen, with
/// `first` run before search is drawn in the same pass, as the window runs its keys before it
/// draws search.
pub(super) fn pass_sized(
    ctx: &Context,
    search: &mut Search,
    state: &AppState,
    events: Vec<Event>,
    height: f32,
    first: impl FnOnce(&Context),
) -> Pass {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(900.0, height));
    let input = RawInput {
        screen_rect: Some(screen),
        events,
        ..RawInput::default()
    };
    let (mut chosen, mut first) = (None, Some(first));
    let mut output = ctx.run_ui(input, |ui| {
        if let Some(first) = first.take() {
            first(ui.ctx());
        }
        chosen = search.show(ui.ctx(), state);
    });
    // No renderer is behind this pass to take the font texture.
    output.textures_delta.clear();
    let mut drawn = Vec::new();
    for clipped in &output.shapes {
        words_of(&clipped.shape, clipped.clip_rect, &mut drawn);
    }
    Pass {
        chosen,
        words: drawn.iter().map(|(word, _, _)| word.clone()).collect(),
        places: drawn
            .iter()
            .map(|(word, place, _)| (word.clone(), *place))
            .collect(),
        clips: drawn.iter().map(|(_, _, clip)| *clip).collect(),
    }
}

/// Draw one pass in a window 900 by 600 points in which `events` happen, with `first` run
/// before search is drawn in the same pass.
pub(super) fn pass_with(
    ctx: &Context,
    search: &mut Search,
    state: &AppState,
    events: Vec<Event>,
    first: impl FnOnce(&Context),
) -> Pass {
    pass_sized(ctx, search, state, events, 600.0, first)
}

/// Draw one pass in which `events` happen.
pub(super) fn pass(
    ctx: &Context,
    search: &mut Search,
    state: &AppState,
    events: Vec<Event>,
) -> Pass {
    pass_with(ctx, search, state, events, |_| {})
}

/// Text typed.
pub(super) fn typed(text: &str) -> Event {
    Event::Text(text.to_owned())
}

/// A key pressed with nothing held.
pub(super) fn pressed(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// A box that is open and has been drawn: once unseen, as egui does with a new area to learn
/// its size, and once as it is seen, so that its field has the keyboard.
pub(super) fn opened(ctx: &Context, state: &AppState) -> Search {
    let mut search = Search::default();
    search.open();
    pass(ctx, &mut search, state, Vec::new());
    pass(ctx, &mut search, state, Vec::new());
    search
}

#[test]
fn no_key_is_taken_in_the_pass_after_the_box_opens() {
    // egui lets go of the focus of a widget in an area it draws unseen: the box says it has the
    // keyboard all the same.
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = Search::default();
    search.open();
    pass(&ctx, &mut search, &state, Vec::new());
    let mut taken = None;
    pass_with(&ctx, &mut search, &state, vec![pressed(Key::H)], |ctx| {
        taken = Some(keys::answered(ctx, &state));
    });
    assert_eq!(taken, Some((Vec::new(), Vec::new())));
    // And the keys of actions are let go once it is closed.
    pass(&ctx, &mut search, &state, vec![pressed(Key::Escape)]);
    assert!(!search.open);
    assert!(!super::holds_keyboard(&ctx));
}

#[test]
fn an_unavailable_command_is_listed_and_says_why() {
    let state = AppState::default();
    let entries = entries_for(&state);
    let listed = ranked("ruler", &entries);
    let ruler = Target::Command("measure.ruler");
    assert!(listed.iter().any(|&place| entries[place].target == ruler));
    // It is drawn, with the reason beneath it.
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, vec![typed("ruler")]);
    let drawn = pass(&ctx, &mut search, &state, Vec::new());
    assert!(
        drawn
            .words
            .iter()
            .any(|word| word.contains("Measure a length"))
    );
    assert!(
        drawn.words.iter().any(|word| word.contains(NO_VOLUME)),
        "{:?}",
        drawn.words
    );
    // Running it posts the reason, and changes nothing else.
    let mut actions = Vec::new();
    let asked = run(&ruler, &state, &mut actions);
    let notice = Notice {
        kind: NoticeKind::Info,
        text: NO_VOLUME.to_owned(),
        details: Vec::new(),
        key: Some("search"),
    };
    assert_eq!((asked, actions), (None, vec![Action::Post(notice)]));
}

#[test]
fn an_action_that_can_be_used_is_not_listed_with_a_reason() {
    let state = thick_scan_ready();
    let ctx = window();
    let mut search = opened(&ctx, &state);
    pass(&ctx, &mut search, &state, vec![typed("ruler")]);
    let drawn = pass(&ctx, &mut search, &state, Vec::new());
    assert!(
        drawn
            .words
            .iter()
            .any(|word| word.contains("Measure a length"))
    );
    assert!(!drawn.words.iter().any(|word| word.contains(NO_VOLUME)));
}

#[test]
fn the_list_is_not_rebuilt_while_nothing_changes() {
    let (state, _) = labelled_head();
    let ctx = window();
    let mut search = Search::default();
    pass(&ctx, &mut search, &state, Vec::new());
    assert!(
        search.entries.is_empty(),
        "nothing is built while the box is closed"
    );
    search.open();
    pass(&ctx, &mut search, &state, Vec::new());
    let (built, at) = (search.built_for, search.entries.as_ptr());
    assert_ne!(search.entries.len(), 0);
    for _ in 0..3 {
        pass(&ctx, &mut search, &state, vec![typed("a")]);
    }
    assert_eq!(search.built_for, built);
    assert_eq!(search.entries.as_ptr(), at, "the same list");
    // A different set of parts is a different list.
    let mut other = state.clone();
    other.labels.parts.truncate(1);
    other.labels.shown.truncate(1);
    pass(&ctx, &mut search, &other, Vec::new());
    assert_ne!(search.built_for, built);
    assert_eq!(
        search
            .entries
            .iter()
            .filter(|entry| entry.heading == Heading::Parts)
            .count(),
        1
    );
}
