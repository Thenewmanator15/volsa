//! Pictures stored in colour.

use volsa_core::{SeriesKind, VolsaError};

use super::*;
use crate::index::{best_series, index};
use crate::testkit::{Colour, MemorySource, SliceOptions, classic_slice};

fn four() -> Vec<[u8; 3]> {
    vec![[200, 10, 20], [10, 190, 30], [20, 30, 180], [240, 240, 240]]
}

/// A 2 by 2 picture stored in colour as the options say.
fn stored(colour: Colour) -> Vec<u8> {
    classic_slice(&SliceOptions {
        rows: 2,
        cols: 2,
        colour: Some(colour),
        ..SliceOptions::default()
    })
}

fn decoded(bytes: &[u8]) -> Result<Frame, VolsaError> {
    ImageFile::open(bytes)?.decode(None)
}

fn refusal(bytes: &[u8]) -> String {
    match decoded(bytes) {
        Err(VolsaError::Decode(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_colour_picture_decodes_to_its_colours_and_their_brightness() {
    let frame = decoded(&stored(Colour::of(four()))).expect("decodes");
    assert_eq!((frame.rows, frame.cols), (2, 2));
    assert_eq!(frame.colour, Some(four()));
    let brightness =
        |[r, g, b]: [u8; 3]| 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b);
    for (value, colour) in frame.values.iter().zip(four()) {
        assert!(
            (value - brightness(colour)).abs() < 0.01,
            "{value} for {colour:?}"
        );
    }
}

#[test]
fn colours_stored_plane_by_plane_come_out_the_same() {
    let planar = Colour {
        planar: true,
        ..Colour::of(four())
    };
    assert_eq!(
        decoded(&stored(planar)).expect("decodes").colour,
        Some(four())
    );
}

#[test]
fn colours_stored_as_brightness_and_two_differences_are_turned_into_red_green_and_blue() {
    // Mid grey, and the strongest red there is, as Y, Cb and Cr.
    let ybr = Colour {
        photometric: "YBR_FULL".to_owned(),
        ..Colour::of(vec![
            [128, 128, 128],
            [76, 85, 255],
            [128, 128, 128],
            [128, 128, 128],
        ])
    };
    let colours = decoded(&stored(ybr))
        .expect("decodes")
        .colour
        .expect("in colour");
    assert_eq!(colours[0], [128, 128, 128]);
    let [red, green, blue] = colours[1];
    assert!(red > 250 && green < 5 && blue < 5, "{:?}", colours[1]);
}

#[test]
fn a_grey_picture_has_no_colours() {
    let frame = decoded(&classic_slice(&SliceOptions::default())).expect("decodes");
    assert_eq!(frame.colour, None);
}

#[test]
fn colour_volsa_cannot_show_is_refused_in_a_sentence() {
    let deep = Colour {
        bits: 16,
        ..Colour::of(four())
    };
    assert!(refusal(&stored(deep)).contains("more than 8 bits"));
    let palette = Colour {
        photometric: "PALETTE COLOR".to_owned(),
        samples: 1,
        ..Colour::of(four())
    };
    assert!(refusal(&stored(palette)).contains("palette colour"));
    let odd = Colour {
        photometric: "YBR_FULL_422".to_owned(),
        ..Colour::of(four())
    };
    assert!(refusal(&stored(odd)).contains("YBR_FULL_422"));
    let short = classic_slice(&SliceOptions {
        rows: 2,
        cols: 2,
        colour: Some(Colour::of(four())),
        drop_pixels: 2,
        ..SliceOptions::default()
    });
    assert!(matches!(decoded(&short), Err(VolsaError::Decode(_))));
}

#[test]
fn a_series_in_colour_is_known_as_one_and_is_never_the_one_opened_by_itself() {
    let files = (0..12)
        .map(|place| {
            let options = SliceOptions {
                rows: 2,
                cols: 2,
                instance_number: place + 1,
                position: Some([0.0, 0.0, f64::from(place)]),
                colour: Some(Colour::of(four())),
                ..SliceOptions::default()
            };
            (format!("I{place:02}"), classic_slice(&options))
        })
        .collect();
    let found = index(&mut MemorySource::new(files), &mut |_| {}).expect("indexes");
    let info = &found.series[0].info;
    assert!(info.colour);
    assert_eq!(info.kind(), SeriesKind::Colour);
    assert_eq!(
        best_series(&found),
        None,
        "a picture is not a scan to open first"
    );
}
