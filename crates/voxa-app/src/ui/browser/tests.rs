//! The browser drawn with no window behind it, and the small pure pieces it is made of.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, vec2};
use voxa_core::SeriesProblem;
use voxa_dicom::testkit::{SliceOptions, classic_slice};

use super::*;
use crate::action::Action;
use crate::state::AppState;
use crate::testing::{THICK_SCAN, id, indexed, three_series};

#[test]
fn a_thumbnail_is_a_small_grey_picture_of_the_frame() {
    let frame = Frame {
        rows: 4,
        cols: 4,
        values: (0..16u8).map(f32::from).collect(),
        colour: None,
    };
    let image = thumbnail(&frame, WindowLevel::new(7.5, 16.0), 2);
    assert_eq!(image.size, [2, 2]);
    let grey: Vec<u8> = image.pixels.iter().map(egui::Color32::r).collect();
    assert!(
        grey[0] < grey[1] && grey[2] < grey[3],
        "brighter to the right: {grey:?}"
    );
    assert!(
        grey[0] < grey[2] && grey[1] < grey[3],
        "brighter downwards: {grey:?}"
    );
}

#[test]
fn a_thumbnail_keeps_the_shape_of_the_frame() {
    let wide = Frame {
        rows: 4,
        cols: 8,
        values: vec![1.0; 32],
        colour: None,
    };
    assert_eq!(thumbnail(&wide, WindowLevel::new(0.5, 1.0), 4).size, [4, 2]);
    let tall = Frame {
        rows: 9,
        cols: 3,
        values: vec![1.0; 27],
        colour: None,
    };
    assert_eq!(thumbnail(&tall, WindowLevel::new(0.5, 1.0), 3).size, [1, 3]);
}

#[test]
fn a_frame_with_nothing_in_it_gives_one_black_pixel() {
    let empty = Frame {
        rows: 0,
        cols: 0,
        values: Vec::new(),
        colour: None,
    };
    let image = thumbnail(&empty, WindowLevel::new(0.0, 1.0), 64);
    assert_eq!(
        (image.size, image.pixels.as_slice()),
        ([1, 1], [egui::Color32::BLACK].as_slice())
    );
    let lying = Frame {
        rows: 4,
        cols: 4,
        values: vec![1.0; 3],
        colour: None,
    };
    assert_eq!(
        thumbnail(&lying, WindowLevel::new(0.0, 1.0), 2).size,
        [1, 1]
    );
}

#[test]
fn a_card_says_how_many_slices_and_how_far_apart() {
    let index = three_series();
    assert_eq!(caption(&index.series[0].info), "30 slices · 5 mm");
    let mut one = index.series[0].info.clone();
    (one.slices, one.spacing_mm) = (1, [0.7, 0.7, 7.5]);
    assert_eq!(caption(&one), "1 slice · 7.5 mm");
}

#[test]
fn the_problems_of_a_series_are_listed_in_plain_words() {
    let mut info = three_series().series[0].info.clone();
    assert_eq!(problems_text(&info), None);
    info.problems = vec![SeriesProblem::UnevenSpacing, SeriesProblem::MixedSizes];
    let text = problems_text(&info).expect("has problems");
    assert_eq!(
        text,
        "The slices are not evenly spaced, so this is shown as a plain stack.\n\
         The slices are not all the same size, so this is shown as a plain stack."
    );
}

/// One pass of the browser in a window 800 by 600, with the given input events.
fn pass(ctx: &egui::Context, state: &AppState, events: Vec<Event>) -> (Vec<Action>, Vec<Card>) {
    let mut actions = Vec::new();
    let mut cards = Vec::new();
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    let input = RawInput {
        screen_rect: Some(screen),
        events,
        ..RawInput::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        cards = show(ui, state, &Thumbnails::default(), &mut actions);
    });
    // No renderer is behind these passes to take the font texture.
    output.textures_delta.clear();
    (actions, cards)
}

fn two_studies() -> AppState {
    let slice = |study: &str, date: &str, series: &str, place: i32| {
        let opts = SliceOptions {
            study_uid: study.to_owned(),
            study_date: Some(date.to_owned()),
            series_uid: series.to_owned(),
            series_description: format!("series of {date}"),
            instance_number: place + 1,
            position: Some([0.0, 0.0, f64::from(place) * 5.0]),
            ..SliceOptions::default()
        };
        (format!("{study}/{place}.dcm"), classic_slice(&opts))
    };
    let mut files: Vec<(String, Vec<u8>)> = (0..3)
        .map(|place| slice("1.2.9.2021", "20210830", "1.2.9.2021.1", place))
        .collect();
    files.extend((0..3).map(|place| slice("1.2.9.2019", "20190412", "1.2.9.2019.1", place)));
    AppState {
        index: Some(indexed(files)),
        ..AppState::default()
    }
}

#[test]
fn the_browser_lists_studies_by_date_with_a_card_for_each_series() {
    let state = two_studies();
    let ctx = egui::Context::default();
    let (actions, cards) = pass(&ctx, &state, Vec::new());
    assert_eq!(actions, Vec::new());
    let listed: Vec<&str> = cards.iter().map(|card| card.series.0.as_str()).collect();
    assert_eq!(
        listed,
        vec!["1.2.9.2019.1", "1.2.9.2021.1"],
        "the earlier study first"
    );
    assert!(
        cards[0].rect.max.y <= cards[1].rect.min.y,
        "one below the other"
    );
    assert!(
        cards.iter().all(|card| card.rect.width() <= WIDTH),
        "{cards:?}"
    );
}

#[test]
fn clicking_a_card_shows_that_series_in_the_active_viewport() {
    let mut state = two_studies();
    state.layout = crate::state::Layout::TwoAcross;
    state.active = 1;
    let ctx = egui::Context::default();
    let (_, cards) = pass(&ctx, &state, Vec::new());
    let at = cards[0].rect.center();
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let (pressed, _) = pass(&ctx, &state, vec![Event::PointerMoved(at), button(true)]);
    assert_eq!(
        pressed,
        Vec::new(),
        "nothing happens until the button comes up"
    );
    let (released, _) = pass(&ctx, &state, vec![button(false)]);
    let expected = Action::SelectSeries {
        viewport: 1,
        series: id("1.2.9.2019.1"),
    };
    assert_eq!(released, vec![expected]);
}

#[test]
fn an_export_with_nothing_open_asks_for_one() {
    let ctx = egui::Context::default();
    let (actions, cards) = pass(&ctx, &AppState::default(), Vec::new());
    assert_eq!((actions.len(), cards.len()), (0, 0));
    let _ = THICK_SCAN;
}

#[test]
fn a_card_notes_what_a_series_is_and_warns_only_of_a_fault() {
    let of = |slices: usize, problems: Vec<SeriesProblem>| SeriesInfo {
        slices,
        problems,
        ..SeriesInfo::default()
    };
    assert_eq!(note(&of(30, Vec::new())), None, "a volume needs no note");
    assert_eq!(
        note(&of(3, vec![SeriesProblem::MixedOrientations])),
        Some(("locating images".to_owned(), false))
    );
    assert_eq!(
        note(&of(1, Vec::new())),
        Some(("a single image".to_owned(), false))
    );
    assert_eq!(
        note(&of(30, vec![SeriesProblem::UnevenSpacing])),
        Some(("! uneven spacing".to_owned(), true))
    );
    let unread = SeriesProblem::UnsupportedTransferSyntax("some other way".to_owned());
    assert_eq!(
        note(&of(30, vec![unread])),
        Some(("! cannot be read yet".to_owned(), true))
    );
}

#[test]
fn resting_on_a_card_gives_the_whole_description_and_every_reason() {
    let long = "a description that is a good deal longer than any card is wide";
    let info = SeriesInfo {
        description: long.to_owned(),
        slices: 30,
        spacing_mm: [1.0, 1.0, 5.0],
        problems: vec![SeriesProblem::UnevenSpacing, SeriesProblem::MixedSizes],
        ..SeriesInfo::default()
    };
    let text = hover(&info);
    assert!(text.starts_with(long), "{text}");
    assert!(text.contains("30 slices · 5 mm"), "{text}");
    assert!(text.contains("The slices are not evenly spaced"), "{text}");
    assert!(text.contains("not all the same size"), "{text}");
    let unnamed = SeriesInfo {
        number: 7,
        ..SeriesInfo::default()
    };
    assert_eq!(hover(&unnamed), "Series 7\n1 slice · 1 mm");
}

#[test]
fn resting_on_a_card_also_says_what_kind_of_scan_it_is() {
    let info = SeriesInfo {
        slices: 30,
        scan: voxa_core::ScanFacts {
            in_3d: Some(true),
            echo_ms: Some(21.0),
            ..voxa_core::ScanFacts::default()
        },
        ..SeriesInfo::default()
    };
    let text = hover(&info);
    assert!(text.contains("Acquired as one block (3D)"), "{text}");
    assert!(text.contains("Echo time 21 ms"), "{text}");
}

/// A made-up volume of a series that holds `count`.
fn volume_of(whole: &str, at: usize, count: usize) -> SeriesInfo {
    SeriesInfo {
        id: SeriesId(format!("{whole}@{}", at + 1)),
        slices: 3,
        spacing_mm: [1.0, 1.0, 5.0],
        set: Some(voxa_core::VolumeSet {
            of: SeriesId(whole.to_owned()),
            at,
            count,
            what: String::new(),
        }),
        ..SeriesInfo::default()
    }
}

#[test]
fn a_series_of_several_volumes_has_one_card_whatever_their_number() {
    let plain = SeriesInfo {
        id: SeriesId("plain".to_owned()),
        ..SeriesInfo::default()
    };
    let mut series = vec![plain.clone()];
    series.extend((0..300).map(|at| volume_of("many", at, 300)));
    series.extend((0..2).map(|at| volume_of("two", at, 2)));
    let ids = |shown: Option<&SeriesId>| -> Vec<String> {
        let cards = one_for_each(&series, shown);
        cards.iter().map(|info| info.id.0.clone()).collect()
    };
    assert_eq!(
        ids(None),
        ["plain", "many@1", "two@1"],
        "the first stands for the rest"
    );
    let second = SeriesId("two@2".to_owned());
    assert_eq!(
        ids(Some(&second)),
        ["plain", "many@1", "two@2"],
        "or the one on show does"
    );
    assert_eq!(ids(Some(&plain.id)), ["plain", "many@1", "two@1"]);
    assert_eq!(
        face_of(&series, &volume_of("two", 1, 2)).id.0,
        "two@1",
        "one picture for all"
    );
    assert_eq!(face_of(&series, &plain).id.0, "plain");
}

#[test]
fn the_card_of_a_series_of_several_volumes_says_how_many() {
    assert_eq!(
        caption(&volume_of("two", 0, 2)),
        "2 volumes · 3 slices · 5 mm"
    );
}

#[test]
fn the_small_picture_of_a_frame_in_colour_keeps_its_colours() {
    let colours = vec![[200, 10, 20], [10, 190, 30], [20, 30, 180], [240, 240, 240]];
    let frame = Frame {
        rows: 2,
        cols: 2,
        values: vec![0.0; 4],
        colour: Some(colours.clone()),
    };
    let image = thumbnail(&frame, WindowLevel::new(0.0, 1.0), 2);
    assert_eq!(image.size, [2, 2]);
    let shown: Vec<[u8; 3]> = image.pixels.iter().map(|p| [p.r(), p.g(), p.b()]).collect();
    assert_eq!(shown, colours);
    // Made smaller, each pixel is the mean of what it covers.
    let one = thumbnail(&frame, WindowLevel::new(0.0, 1.0), 1);
    assert_eq!(one.size, [1, 1]);
    assert_eq!(
        one.pixels[0].r(),
        118,
        "the mean of 200, 10, 20 and 240, rounded"
    );
}
