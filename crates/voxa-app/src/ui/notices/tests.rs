//! The line along the bottom of the window: where its notice goes, and the list of them all.

use egui::{Id, Pos2, Rect};

use super::*;
use crate::ui::seen::{self, Seen, click_at, is_red, key};
use crate::ui::theme::ThemeChoice;

/// The line drawn whole, as the window draws it in two steps: the line, then the notice area.
/// A click that asks for the list to be opened opens it. Gives the disclaimer.
fn show(ui: &mut Ui, notices: &[Notice], open: &mut bool, actions: &mut Vec<Action>) -> Response {
    let state = AppState {
        notices: notices.to_vec(),
        ..AppState::default()
    };
    show_state(ui, &state, open, actions)
}

/// As [`show`], for the notices of a state, which can name the patients of the export open.
pub(super) fn show_state(
    ui: &mut Ui,
    state: &AppState,
    open: &mut bool,
    actions: &mut Vec<Action>,
) -> Response {
    let line = place(ui, state);
    for request in draw(ui, state, &line, open, actions) {
        if request == Request::ShowNotices {
            *open = true;
        }
    }
    line.disclaimer
}

/// A notice of this kind and these words.
fn notice(kind: NoticeKind, text: &str) -> Notice {
    Notice {
        kind,
        text: text.to_owned(),
        details: Vec::new(),
        key: None,
    }
}

/// A notice long enough to run across the whole of a narrow window.
fn long_notice() -> Notice {
    notice(
        NoticeKind::Warning,
        concat!(
            "A notice long enough to run across the whole of a narrow window and on ",
            "past the disclaimer, which it must not cover"
        ),
    )
}

/// What a pass of the line drew, in a window `width` points across.
fn drawn(width: f32, notices: &[Notice]) -> Seen {
    let ctx = seen::context(1.0);
    let mut open = false;
    let mut actions = Vec::new();
    let ((), seen) = seen::pass(&ctx, [width, 480.0], Vec::new(), |ui| {
        show(ui, notices, &mut open, &mut actions);
    });
    seen
}

/// Fails if any two of the words cover one another.
fn apart(words: &[(String, Rect)]) {
    for (one, (said, rect)) in words.iter().enumerate() {
        for (other, there) in words.iter().skip(one + 1) {
            assert!(!rect.intersects(*there), "{said:?} covers {other:?}");
        }
    }
}

#[test]
fn a_long_notice_in_a_narrow_window_is_cut_short_beside_the_disclaimer() {
    let seen = drawn(640.0, &[long_notice()]);
    assert_eq!(seen.words.len(), 2, "the disclaimer and the notice");
    let lines = seen.words.iter().map(|(_, rect)| rect.center().y.round());
    let first = seen.words[0].1.center().y.round();
    assert!(
        lines.clone().all(|line| (line - first).abs() <= 2.0),
        "one line: {:?}",
        seen.words
    );
    apart(&seen.words);
    // The notice is cut short: it takes less room than it does in a window wide enough for it.
    let whole = drawn(2000.0, &[long_notice()]);
    let (cut, whole) = (seen.words[1].1.width(), whole.words[1].1.width());
    assert!(cut < whole - 20.0, "{cut} against {whole}");
}

#[test]
fn with_no_room_beside_the_disclaimer_the_notice_goes_on_a_line_below_it() {
    // The smallest window with the interface at twice its usual size.
    let seen = drawn(320.0, &[long_notice()]);
    assert_eq!(seen.words.len(), 2, "the disclaimer and the notice");
    let disclaimer = seen
        .words
        .iter()
        .find(|(said, _)| said == theme::DISCLAIMER);
    let (_, disclaimer) = disclaimer.expect("the disclaimer, whole");
    for (said, rect) in &seen.words {
        if said != theme::DISCLAIMER {
            assert!(rect.top() >= disclaimer.bottom(), "{said:?} is beside it");
        }
    }
    apart(&seen.words);
}

#[test]
fn with_no_notice_the_disclaimer_wraps_in_a_window_too_narrow_for_it() {
    let ctx = seen::context(1.0);
    let mut open = true;
    let (shown, seen) = seen::pass(&ctx, [160.0, 480.0], Vec::new(), |ui| {
        show(ui, &[], &mut open, &mut Vec::new())
    });
    assert!(shown.rect.right() <= 160.0, "{:?}", shown.rect);
    assert!(!open, "with nothing to list there is no list");
    let said: Vec<&str> = seen.words.iter().map(|(said, _)| said.as_str()).collect();
    assert_eq!(said, [theme::DISCLAIMER], "every word of it");
}

#[test]
fn the_area_says_how_many_notices_there_are_when_there_is_more_than_one() {
    let one = [notice(NoticeKind::Info, "Opened 12 series.")];
    let two = [
        one[0].clone(),
        notice(NoticeKind::Warning, "One file was skipped."),
    ];
    let alone = drawn(900.0, &one);
    assert!(
        alone
            .words
            .iter()
            .all(|(said, _)| !said.contains("notices"))
    );
    let both = drawn(900.0, &two);
    let counted = both.words.iter().filter(|(said, _)| said == "2 notices");
    assert_eq!(counted.count(), 1, "{:?}", both.words);
    // The newest is the one shown on the line.
    assert!(
        both.words
            .iter()
            .any(|(said, _)| said == "One file was skipped.")
    );
    assert!(
        both.words
            .iter()
            .all(|(said, _)| said != "Opened 12 series.")
    );
}

/// Three notices, oldest first, of the three kinds.
fn three() -> Vec<Notice> {
    vec![
        notice(NoticeKind::Info, "Opened 12 series."),
        notice(NoticeKind::Warning, "One file was skipped."),
        notice(NoticeKind::Error, "A slice could not be read."),
    ]
}

/// The line with these notices and its list open, after the passes the list needs to appear:
/// the context, what was drawn last and the actions it gave.
fn opened(notices: &[Notice], size: [f32; 2]) -> (Context, Seen, Vec<Action>) {
    opened_in(ThemeChoice::Dark, notices, size)
}

/// [`opened`], in a window drawn in this theme.
fn opened_in(
    choice: ThemeChoice,
    notices: &[Notice],
    size: [f32; 2],
) -> (Context, Seen, Vec<Action>) {
    let ctx = seen::context_in(1.0, choice);
    let mut open = true;
    let mut actions = Vec::new();
    let mut last = Seen::default();
    // An area is laid out unseen once, to learn its size, before it is drawn.
    for _ in 0..3 {
        actions.clear();
        let ((), seen) = seen::pass(&ctx, size, Vec::new(), |ui| {
            egui::Panel::bottom("line").show(ui, |ui| show(ui, notices, &mut open, &mut actions));
        });
        last = seen;
    }
    assert!(open);
    (ctx, last, actions)
}

#[test]
fn the_notice_list_shows_every_notice_newest_first_with_dismiss() {
    let notices = three();
    let (ctx, seen, _) = opened(&notices, [900.0, 600.0]);
    // Every notice is in the list, the newest at the top.
    let place = |text: &str| {
        let found = seen.words.iter().filter(|(said, _)| said == text);
        let found: Vec<&Rect> = found.map(|(_, rect)| rect).collect();
        found.last().copied().copied()
    };
    let tops: Vec<f32> = notices
        .iter()
        .map(|notice| place(&notice.text).expect("it is listed").top())
        .collect();
    assert!(
        tops[2] < tops[1] && tops[1] < tops[0],
        "newest first: {tops:?}"
    );
    // And each has its own Dismiss.
    let dismissals = seen.places_starting("Dismiss: ");
    assert_eq!(dismissals.len(), 3, "{:?}", seen.names);
    // The list is above the line, which has the disclaimer.
    let line = seen
        .words
        .iter()
        .find(|(said, _)| said == theme::DISCLAIMER);
    let line = line.expect("the disclaimer").1;
    assert!(tops.iter().all(|top| *top < line.top()), "above the line");
    // Dismissing the middle one gives its place among them, counted from the oldest.
    let middle = seen
        .place_of("Dismiss: One file was skipped.")
        .expect("its Dismiss");
    let [pressed, released] = click_at(middle.center());
    let mut open = true;
    let mut actions = Vec::new();
    for events in [pressed, released] {
        actions.clear();
        seen::pass(&ctx, [900.0, 600.0], events, |ui| {
            egui::Panel::bottom("line").show(ui, |ui| show(ui, &notices, &mut open, &mut actions));
        });
    }
    assert_eq!(actions, [Action::DismissNotice(1)]);
    assert!(
        open,
        "the list stays open while there are notices to dismiss"
    );
}

#[test]
fn the_list_closes_with_esc_or_a_click_anywhere_else_and_opens_with_a_click_on_the_area() {
    let notices = three();
    let (ctx, seen, _) = opened(&notices, [900.0, 600.0]);
    let size = [900.0, 600.0];
    let run = |ctx: &Context, events: Vec<egui::Event>, open: &mut bool| {
        seen::pass(ctx, size, events, |ui| {
            egui::Panel::bottom("line").show(ui, |ui| show(ui, &notices, open, &mut Vec::new()));
        })
        .1
    };
    // Esc closes it.
    let mut open = true;
    run(&ctx, vec![key(egui::Key::Escape)], &mut open);
    assert!(!open, "Esc");
    // A press on the window, away from the list and the area, closes it.
    let mut open = true;
    let away = Pos2::new(20.0, 20.0);
    run(&ctx, click_at(away)[0].clone(), &mut open);
    assert!(!open, "a click elsewhere");
    // A click on the area opens it, and another closes it.
    let area = seen
        .place_starting("3 notices")
        .expect("the area is a button");
    let mut open = false;
    for events in click_at(area.center()) {
        run(&ctx, events, &mut open);
    }
    assert!(open, "a click on the area opens the list");
    for events in click_at(area.center()) {
        run(&ctx, events, &mut open);
    }
    assert!(!open, "and a second click closes it");
}

#[test]
fn no_notice_is_drawn_red() {
    // Of every kind, on the line alone and in the list.
    let kinds = [NoticeKind::Info, NoticeKind::Warning, NoticeKind::Error];
    for kind in kinds {
        let notices = [notice(kind, "Something to say.")];
        let line = drawn(900.0, &notices);
        assert!(!line.colours.is_empty(), "{kind:?} is drawn");
        assert!(
            line.colours.iter().all(|colour| !is_red(*colour)),
            "{kind:?}"
        );
        let (_, listed, _) = opened(&notices, [900.0, 600.0]);
        assert!(listed.colours.len() > 3, "{kind:?} is drawn in the list");
        assert!(
            listed.colours.iter().all(|colour| !is_red(*colour)),
            "{kind:?} listed"
        );
    }
    // An error is drawn as something to look at: the same amber as a warning, in each theme.
    for choice in [ThemeChoice::Dark, ThemeChoice::Light] {
        let ctx = seen::context_in(1.0, choice);
        let amber = theme::palette(&ctx).warning;
        let drawn_in = |kind| look(&ctx, kind).1;
        assert_eq!(drawn_in(NoticeKind::Error), amber, "{choice:?}");
        assert_eq!(drawn_in(NoticeKind::Warning), amber, "{choice:?}");
        assert_ne!(drawn_in(NoticeKind::Info), amber, "{choice:?}");
        assert!(!is_red(amber), "{choice:?}");
    }
    // And the test itself knows red when it sees it.
    assert!(is_red(egui::Color32::from_rgb(220, 40, 40)));
}

#[test]
fn the_line_and_its_list_read_in_both_themes() {
    for choice in [ThemeChoice::Dark, ThemeChoice::Light] {
        let (ctx, listed, _) = opened_in(choice, &three(), [900.0, 600.0]);
        let palette = theme::palette(&ctx);
        assert!(listed.words.len() > 6, "the line and the list are drawn");
        // The list is a window of the theme's panel colour.
        assert!(listed.drew(palette.panel), "{choice:?}");
        for ground in [palette.panel, palette.raised] {
            let faint = listed.hard_to_read_on(ground, &[]);
            assert!(faint.is_empty(), "{choice:?} on {ground:?}: {faint:?}");
        }
    }
}

#[test]
fn the_list_keeps_off_the_scans_rail_in_a_narrow_window() {
    // A window 700 points across with the rail open: the list, up to 460 points wide and
    // hung from the right, would reach 232 and cover the rail's last 28 points.
    let notices = three();
    let size = [700.0, 500.0];
    let rail = 260.0;
    let ctx = seen::context(1.0);
    let mut open = true;
    let mut laid = Rect::NOTHING;
    // The window lays out the line, then the rail, then the pictures, which tell the floating
    // windows where they are: the list reads that, from the frame before.
    for _ in 0..4 {
        let (rail_at, _) = seen::pass(&ctx, size, Vec::new(), |ui| {
            egui::Panel::bottom("line").show(ui, |ui| {
                show(ui, &notices, &mut open, &mut Vec::new());
            });
            let column = egui::Panel::left("rail").exact_size(rail);
            let rail_at = column.show(ui, |ui| ui.allocate_space(ui.available_size()));
            let middle = egui::CentralPanel::default().show(ui, |ui| {
                ui.allocate_space(ui.available_size());
            });
            crate::ui::floating::keep_to(ui.ctx(), middle.response.rect);
            rail_at.response.rect
        });
        laid = rail_at;
    }
    assert!(open);
    let list = ctx.memory(|memory| memory.area_rect(Id::new(LIST)));
    let list = list.expect("the list is shown");
    assert!(
        list.left() >= laid.right() && !list.intersect(laid).is_positive(),
        "the list at {list:?} meets the rail at {laid:?}"
    );
    assert!(list.right() <= size[0], "{list:?} is inside the window");
    assert!(
        list.width() > 200.0,
        "and not squeezed to nothing: {list:?}"
    );
}
