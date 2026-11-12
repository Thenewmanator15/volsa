//! The top of a document in the middle of the window: a title of any length leaves room for the
//! two buttons beside it.

use egui::{Context, Pos2, Rect, vec2};
use volsa_dicom::Report;

use super::tests::opened;
use super::*;
use crate::testing::documents::REPORT;
use crate::ui::seen::{self, Seen};

/// A made-up title of 300 characters, far longer than any room it has.
fn long_title() -> String {
    "A made-up title that runs on and on, ".repeat(9)[..300].to_owned()
}

/// A pass of the document drawn in an area `wide` by 400 points, whose title is `title`.
fn pass_in(ctx: &Context, wide: f32, title: &str) -> (Rect, Seen) {
    let mut state = opened(
        REPORT,
        Some(Ok(Report {
            lines: Vec::new(),
            cut_short: false,
        })),
    );
    let index = state.index.as_mut().expect("an export is open");
    let report = index.documents.iter_mut().find(|one| one.entry == REPORT);
    report.expect("the report is listed").title = title.to_owned();
    let open = state.document.clone().expect("a document is open");
    let area = Rect::from_min_size(Pos2::ZERO, vec2(wide, 400.0));
    let draw = |ui: &mut Ui| {
        show(
            ui,
            area,
            &state,
            &open,
            &mut Kept::default(),
            &mut Vec::new(),
        )
    };
    let (_, seen) = seen::pass(ctx, [wide, 400.0], Vec::new(), draw);
    (area, seen)
}

/// What a screen reader calls the button that closes the document.
fn closing() -> &'static str {
    commands::find("document.close").expect("listed").name
}

#[test]
fn a_title_of_300_characters_in_300_points_leaves_both_buttons_whole_and_clear_of_it() {
    let ctx = seen::context(1.0);
    let title = long_title();
    assert_eq!(title.chars().count(), 300);
    // Drawn twice, for the room the buttons take to be known.
    let _ = pass_in(&ctx, 300.0, &title);
    let (area, seen) = pass_in(&ctx, 300.0, &title);
    let tags = commands::find("tags.show").expect("listed").name;
    let buttons = [tags, closing()].map(|name| {
        let place = seen.place_of(name);
        place.unwrap_or_else(|| panic!("{name} is drawn: {:?}", seen.names))
    });
    for (name, place) in [tags, closing()].into_iter().zip(buttons) {
        assert!(
            area.contains_rect(place),
            "{name} at {place:?} is outside {area:?}"
        );
    }
    assert!(
        !buttons[0].intersect(buttons[1]).is_positive(),
        "the buttons overlap: {buttons:?}"
    );
    // The title is cut with an ellipsis, and what is left of it ends before the first button.
    let (_, title_at) = seen
        .words
        .iter()
        .find(|(word, _)| word.contains("A made-up title"))
        .expect("the title is drawn");
    assert!(!seen.cut.is_empty(), "the title is cut short to fit");
    let first_button = buttons[0].left().min(buttons[1].left());
    assert!(
        title_at.right() <= first_button + 0.5,
        "the title at {title_at:?} runs under the buttons at {buttons:?}"
    );
    // The kind of document, above it, is whole too.
    let (_, kind_at) = seen
        .words
        .iter()
        .find(|(word, _)| word == "Report")
        .expect("drawn");
    assert!(area.contains_rect(*kind_at) && kind_at.right() <= first_button + 0.5);
}

#[test]
fn a_short_title_leaves_the_buttons_at_the_right_edge() {
    let ctx = seen::context(1.0);
    let _ = pass_in(&ctx, 600.0, "Made-up report");
    let (area, seen) = pass_in(&ctx, 600.0, "Made-up report");
    let close = seen.place_of(closing()).expect("drawn");
    assert!(area.right() - close.right() < 24.0, "{close:?} in {area:?}");
}
