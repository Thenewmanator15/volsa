//! The small pure pieces the rail of scans is made of: thumbnails, the words of a card and a
//! study's heading.

use volsa_core::SeriesProblem;

use super::*;
use crate::testing::three_series;

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
    assert_eq!(
        caption(&one),
        "1 slice",
        "one slice is no distance from another"
    );
    // Said only when the files gave it, however many slices: a stack with no positions and no
    // thickness has a stand-in of 1 mm, and a thickness is a distance the files did give.
    let mut stack = index.series[0].info.clone();
    (stack.slice_step_given, stack.spacing_mm[2]) = (false, 1.0);
    assert_eq!(caption(&stack), "30 slices");
    (stack.slice_step_given, stack.spacing_mm[2]) = (true, 4.0);
    assert_eq!(caption(&stack), "30 slices · 4 mm");
    let mut pictures = one.clone();
    (pictures.slices, pictures.colour) = (2, true);
    assert_eq!(
        caption(&pictures),
        "2 pictures",
        "a picture has no thickness"
    );
    pictures.slices = 1;
    assert_eq!(caption(&pictures), "1 picture");
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

#[test]
fn resting_on_a_card_gives_the_whole_description_and_every_reason() {
    let long = "a description that is a good deal longer than any card is wide";
    let info = SeriesInfo {
        description: long.to_owned(),
        slices: 30,
        spacing_mm: [1.0, 1.0, 5.0],
        slice_step_given: true,
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
    assert_eq!(
        hover(&unnamed),
        "Series 7\n1 slice",
        "the 1 mm of a series the files said nothing of is not a distance"
    );
}

#[test]
fn resting_on_a_card_also_says_what_kind_of_scan_it_is() {
    let info = SeriesInfo {
        slices: 30,
        scan: volsa_core::ScanFacts {
            in_3d: Some(true),
            echo_ms: Some(21.0),
            ..volsa_core::ScanFacts::default()
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
        slice_step_given: true,
        set: Some(volsa_core::VolumeSet {
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
        cards.iter().map(|at| series[*at].id.0.clone()).collect()
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

#[test]
fn no_small_picture_of_a_page_is_made_while_the_patients_details_are_hidden() {
    let of = |modality: &str| SeriesInfo {
        modality: modality.to_owned(),
        ..SeriesInfo::default()
    };
    for page in ["SC", "OT", "DOC", " sc ", "RTIMAGE", "RTDOSE", "rtimage"] {
        assert!(!may_have_thumbnail(&of(page), false), "{page}");
        assert!(may_have_thumbnail(&of(page), true), "{page}");
    }
    for scan in ["MR", "CT", "US", "", "SEG", "PR", "KO"] {
        assert!(may_have_thumbnail(&of(scan), false), "{scan}");
    }
}
