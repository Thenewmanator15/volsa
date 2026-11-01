//! A card of the rail: its words, its mark, its picture and its menu.

use egui::{Color32, ColorImage, Event, Modifiers, PointerButton, Rect, TextureOptions};
use volsa_core::{SeriesInfo, SeriesProblem};

use super::tests::{
    Drawn, WIDE, card_at, click, export, faulted, pass, pass_wide, series, top_of, unreadable,
};
use super::*;
use crate::action::Action;
use crate::naming::plain_name;
use crate::state::Layout;
use crate::ui::commands::TagsOf;
use crate::ui::seen::{self, Seen};

/// The words drawn inside `rect`, from the top down.
fn words_in(seen: &Seen, rect: Rect) -> Vec<(String, Rect)> {
    let inside = seen
        .words
        .iter()
        .filter(|(_, at)| rect.contains(at.center()));
    inside.cloned().collect()
}

#[test]
fn a_card_shows_the_plain_name_and_the_description_beneath() {
    let named = series(1, "MR", "made-up t2 ax", 30);
    let unnamed = series(7, "MR", "", 30);
    let state = export(vec![(
        "MADE-UP-1",
        "Made^Up",
        vec![named.clone(), unnamed.clone()],
    )]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let card = card_at(&drawn.seen, &named);
    let words = words_in(&drawn.seen, card);
    let said: Vec<&str> = words.iter().map(|(word, _)| word.as_str()).collect();
    assert_eq!(said, ["T2, axial", "made-up t2 ax", "30 slices · 5 mm"]);
    let (name, description) = (words[0].1, words[1].1);
    assert!(
        name.bottom() <= description.top(),
        "beneath: {name:?} {description:?}"
    );
    assert!(description.height() < name.height(), "in smaller words");
    // With no description, the series' number stands in its place.
    let card = card_at(&drawn.seen, &unnamed);
    let said: Vec<String> = words_in(&drawn.seen, card)
        .into_iter()
        .map(|(w, _)| w)
        .collect();
    assert_eq!(said[1], "Series 7", "{said:?}");
    // Resting on a card gives the plain name first, then the description whole.
    let hover = card::hover_of(&named);
    assert!(
        hover.starts_with("T2, axial\nmade-up t2 ax\n30 slices"),
        "{hover}"
    );
}

#[test]
fn a_faulted_series_has_a_mark_and_no_warning_words_in_the_list() {
    let uneven = faulted(1, "made-up t2 ax", SeriesProblem::UnevenSpacing);
    let planes = faulted(2, "made-up t1", SeriesProblem::MixedOrientations);
    let unread = faulted(3, "made-up flair", unreadable());
    let clean = series(4, "MR", "made-up pd", 30);
    let all = [&uneven, &planes, &unread, &clean].map(Clone::clone);
    let state = export(vec![("MADE-UP-1", "Made^Up", all.to_vec())]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let marks = drawn.seen.places_starting(card::NOT_SHOWN_IN_FULL);
    let marks_on = |info: &SeriesInfo| {
        let card = card_at(&drawn.seen, info);
        let bang = words_in(&drawn.seen, card)
            .into_iter()
            .any(|(word, _)| word == "!");
        let marked = marks
            .iter()
            .filter(|mark| card.contains_rect(**mark))
            .count();
        (marked, bang)
    };
    for info in [&uneven, &planes, &unread] {
        assert_eq!(marks_on(info), (1, true), "{}", info.description);
    }
    assert_eq!(marks_on(&clean), (0, false), "no mark on a clean scan");
    let colours = &drawn.seen.colours;
    assert!(colours.contains(&theme::WARNING), "the mark is amber");
    assert!(
        !colours.iter().any(|colour| seen::is_red(*colour)),
        "nothing red"
    );
    // The words of a problem are in the hover text, not on the card.
    let problem = [
        "several planes",
        "uneven",
        "plain stack",
        "cannot be read",
        "same way",
    ];
    for words in problem {
        let found = drawn
            .seen
            .words
            .iter()
            .find(|(word, _)| word.contains(words));
        assert_eq!(found, None, "{words}");
    }
    assert!(card::hover_of(&uneven).contains("The slices are not evenly spaced"));
    assert!(card::hover_of(&unread).contains("cannot read yet (made-up)"));
}

#[test]
fn pages_show_an_icon_not_a_thumbnail_while_details_are_off() {
    let page = series(1, "SC", "made-up page", 1);
    let scan = series(2, "MR", "made-up t1", 20);
    let mut state = export(vec![(
        "MADE-UP-1",
        "Made^Up",
        vec![page.clone(), scan.clone()],
    )]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    rail.other_open = true;
    let picture = |name: &str| {
        let grey = ColorImage::filled([2, 2], Color32::GRAY);
        ctx.load_texture(name, grey, TextureOptions::default())
    };
    let mut thumbnails = Thumbnails::default();
    thumbnails.insert(page.id.clone(), picture("made-up page"));
    thumbnails.insert(scan.id.clone(), picture("made-up scan"));
    let ids = |drawn: &Drawn, info: &SeriesInfo| {
        let card = card_at(&drawn.seen, info);
        let on = drawn
            .seen
            .images
            .iter()
            .filter(|(_, at)| card.contains_rect(*at));
        on.map(|(texture, _)| *texture).collect::<Vec<_>>()
    };
    let drawn = pass_wide(&ctx, &state, &mut rail, &thumbnails, WIDE, Vec::new());
    assert_eq!(ids(&drawn, &page), [], "no picture of the page");
    assert_eq!(
        ids(&drawn, &scan),
        [thumbnails[&scan.id].id()],
        "the scan's own"
    );
    state.show_patient = true;
    let drawn = pass_wide(&ctx, &state, &mut rail, &thumbnails, WIDE, Vec::new());
    assert_eq!(
        ids(&drawn, &page),
        [thumbnails[&page.id].id()],
        "shown with the details"
    );
}

#[test]
fn radiotherapy_images_and_doses_show_an_icon_too_while_details_are_off() {
    let image = series(1, "RTIMAGE", "made-up portal image", 1);
    let dose = series(2, "RTDOSE", "made-up dose", 1);
    let segmentation = series(3, "SEG", "made-up segmentation", 1);
    let mut state = export(vec![(
        "MADE-UP-1",
        "Made^Up",
        vec![image.clone(), dose.clone(), segmentation.clone()],
    )]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    rail.other_open = true;
    let mut thumbnails = Thumbnails::default();
    for info in [&image, &dose, &segmentation] {
        let grey = ColorImage::filled([2, 2], Color32::GRAY);
        let texture = ctx.load_texture(&info.description, grey, TextureOptions::default());
        thumbnails.insert(info.id.clone(), texture);
    }
    let pictures = |drawn: &Drawn, info: &SeriesInfo| {
        let card = card_at(&drawn.seen, info);
        let images = drawn.seen.images.iter();
        images.filter(|(_, at)| card.contains_rect(*at)).count()
    };
    let drawn = pass_wide(&ctx, &state, &mut rail, &thumbnails, WIDE, Vec::new());
    assert_eq!(
        pictures(&drawn, &image),
        0,
        "a portal image may carry a name"
    );
    assert_eq!(pictures(&drawn, &dose), 0, "so may a dose");
    assert_eq!(
        pictures(&drawn, &segmentation),
        1,
        "a segmentation has none to carry"
    );
    state.show_patient = true;
    let drawn = pass_wide(&ctx, &state, &mut rail, &thumbnails, WIDE, Vec::new());
    assert_eq!(pictures(&drawn, &image), 1, "shown with the details");
    assert_eq!(pictures(&drawn, &dose), 1);
}

#[test]
fn a_long_name_is_cut_and_never_widens_the_rail() {
    let long: String = "made-up words of a scanner that go on ".repeat(8);
    let long = long[..300].to_owned();
    let mut described = series(1, "MR", &long, 30);
    described.scan.body_part = "made up body part ".repeat(10);
    let short = series(2, "MR", "made-up t1", 30);
    let state = export(vec![(
        "MADE-UP-1",
        "Made^Up",
        vec![described.clone(), short.clone()],
    )]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let narrow = 160.0;
    let drawn = pass_wide(
        &ctx,
        &state,
        &mut rail,
        &Thumbnails::default(),
        narrow,
        Vec::new(),
    );
    assert!(
        drawn.rect.width() <= narrow + 0.5,
        "the rail is as wide as it was: {:?}",
        drawn.rect
    );
    let card = card_at(&drawn.seen, &described);
    assert!(
        card.right() <= drawn.rect.right() + 0.5,
        "{card:?} in {:?}",
        drawn.rect
    );
    let words = words_in(&drawn.seen, card);
    for (word, at) in &words {
        assert!(
            at.right() <= card.right(),
            "{word:?} runs past the card: {at:?} {card:?}"
        );
    }
    // Both the description and the name are cut short with an ellipsis.
    let cut_from = |whole: &str| {
        drawn.seen.cut.iter().any(|drawn| {
            let kept = drawn.trim_end_matches('…');
            drawn.ends_with('…') && !kept.is_empty() && whole.starts_with(kept)
        })
    };
    assert!(cut_from(&long), "the description: {:?}", drawn.seen.cut);
    let name = plain_name(&described);
    assert!(cut_from(&name), "the name: {:?}", drawn.seen.cut);
    // It does not reach the card under it, and its hover text says all of it.
    let next = top_of(&drawn.seen, &short).expect("drawn");
    assert!(card.bottom() < next, "{card:?} meets {next}");
    let hover = card::hover_of(&described);
    assert!(hover.contains(&long) && hover.contains(&name), "{hover}");
}

#[test]
fn a_cards_menu_shows_it_or_its_tags() {
    let (scan, other) = (
        series(1, "MR", "made-up t1", 20),
        series(2, "MR", "made-up t2", 20),
    );
    let mut state = export(vec![(
        "MADE-UP-1",
        "Made^Up",
        vec![scan.clone(), other.clone()],
    )]);
    (state.layout, state.active) = (Layout::TwoAcross, 1);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let tags = commands::find("tags.show").expect("listed").name;
    // By the other button: the menu's two items, each doing what it says.
    let shown = Action::SelectSeries {
        viewport: 1,
        series: scan.id.clone(),
    };
    let asked = Request::Tags(TagsOf::Series(scan.id.clone()));
    let items = [
        (card::SHOW_HERE, (vec![shown], vec![])),
        (tags, (vec![], vec![asked])),
    ];
    for (item, expected) in items {
        let (opened, card) = menu_of(&ctx, &state, &mut rail, &scan);
        let found = opened.seen.place_of(item).expect("in the menu");
        assert!(!card.contains(found.center()), "{found:?} on {card:?}");
        let chosen = click(&ctx, &state, &mut rail, found);
        assert_eq!((chosen.actions, chosen.requests), expected, "{item}");
    }
    // By its ⋯, named for a screen reader by the card's own name.
    let drawn = pass(&ctx, &state, &mut rail, Vec::new());
    let dots = card::dots_name(&card::name_of(&other));
    let button = drawn.seen.place_of(&dots).expect("named");
    let clicked = click(&ctx, &state, &mut rail, button);
    assert_eq!(
        (clicked.actions, clicked.requests),
        (vec![], vec![]),
        "only opens it"
    );
    let opened = pass(&ctx, &state, &mut rail, Vec::new());
    let found = opened.seen.place_of(tags).expect("in the menu");
    let chosen = click(&ctx, &state, &mut rail, found);
    let asked = Request::Tags(TagsOf::Series(other.id.clone()));
    assert_eq!(chosen.requests, [asked]);
}

/// The two passes' events of pressing and letting go of the other button at `at`.
pub(super) fn right_click(at: egui::Pos2) -> [Vec<Event>; 2] {
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    [
        vec![Event::PointerMoved(at), button(true)],
        vec![button(false)],
    ]
}

/// Open the menu of the card of `info` with the other button, near the card's corner so that
/// the menu opens off the card and a click on an item is no click on the card. Gives the pass
/// once the menu has taken its place, which it does a pass after it opens, and where the card
/// is.
fn menu_of(
    ctx: &egui::Context,
    state: &AppState,
    rail: &mut RailState,
    info: &SeriesInfo,
) -> (Drawn, Rect) {
    let drawn = pass(ctx, state, rail, Vec::new());
    let card = card_at(&drawn.seen, info);
    let [press, release] = right_click(card.right_bottom() - egui::vec2(6.0, 6.0));
    let _ = pass(ctx, state, rail, press);
    let _ = pass(ctx, state, rail, release);
    let opened = pass(ctx, state, rail, Vec::new());
    assert!(
        opened.seen.place_of(card::SHOW_HERE).is_some(),
        "{:?}",
        opened.seen.names
    );
    (opened, card)
}

#[test]
fn a_card_that_has_the_keyboard_is_ringed_in_the_themes_colour_of_focus() {
    let scan = series(1, "MR", "made-up t2 ax", 30);
    let state = export(vec![("MADE-UP-1", "Made^Up", vec![scan.clone()])]);
    let (ctx, mut rail) = (seen::context(1.0), RailState::default());
    let ring = ctx.global_style().visuals.selection.stroke.color;
    let rings = |drawn: &Drawn| {
        drawn
            .seen
            .colours
            .iter()
            .filter(|colour| **colour == ring)
            .count()
    };
    let plain = pass(&ctx, &state, &mut rail, Vec::new());
    ctx.memory_mut(|memory| memory.request_focus(card::id_of(&scan.id)));
    let _ = pass(&ctx, &state, &mut rail, Vec::new());
    let focused = pass(&ctx, &state, &mut rail, Vec::new());
    assert!(
        rings(&focused) > rings(&plain),
        "{} / {}",
        rings(&focused),
        rings(&plain)
    );
}
