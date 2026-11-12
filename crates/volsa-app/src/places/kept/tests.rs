//! The file of places: what it holds, how it is bounded, and what becomes of one that cannot
//! be read.

use super::*;
use volsa_core::SeriesId;

use crate::kept_file::aside;
use crate::views::series_key;

fn place(slice: usize) -> Place {
    Place {
        slice,
        window: Some([40.0, 400.0]),
        zoom: 2.0,
        pan: [3.0, -4.0],
        flip_h: true,
        flip_v: false,
        turns: 2,
    }
}

fn keep(folder: &Path, key: &str, slice: usize) {
    save(folder, &[(key.to_owned(), place(slice))], None).expect("saves");
}

#[test]
fn places_that_are_saved_are_loaded_again_and_those_of_other_series_are_left() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let deeper = folder.path().join("not").join("there yet");
    assert_eq!(load(&deeper), Kept::default(), "no file holds nothing");
    keep(&deeper, "one", 1);
    let both = [("two".to_owned(), place(2)), ("three".to_owned(), place(3))];
    save(&deeper, &both, Some(("a study", "two"))).expect("saves");
    keep(&deeper, "one", 11);
    let kept = load(&deeper);
    assert_eq!(kept.place("one"), Some(place(11)), "the later of the two");
    assert_eq!(kept.place("two"), Some(place(2)));
    assert_eq!(kept.place("three"), Some(place(3)));
    assert_eq!(kept.place("four"), None);
    assert_eq!(kept.first(&["a study".to_owned()]), Some("two"));
    assert_eq!(kept.first(&["another study".to_owned()]), None);
}

#[test]
fn of_several_studies_the_first_series_of_the_one_written_last_is_given() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    save(folder.path(), &[], Some(("early", "one"))).expect("saves");
    save(folder.path(), &[], Some(("late", "two"))).expect("saves");
    let kept = load(folder.path());
    let studies = ["early", "late", "never seen"].map(str::to_owned);
    assert_eq!(kept.first(&studies), Some("two"));
    save(folder.path(), &[], Some(("early", "three"))).expect("saves");
    assert_eq!(load(folder.path()).first(&studies), Some("three"));
}

#[test]
fn the_file_holds_keys_and_numbers_and_nothing_else() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    // The keys are hashes of made-up identifiers, as Volsa files them.
    let key = series_key(&SeriesId("made-up series".to_owned()));
    let study = series_key(&SeriesId("made-up study".to_owned()));
    let kept = [(key.clone(), place(5))];
    save(folder.path(), &kept, Some((study.as_str(), key.as_str()))).expect("saves");
    let text = std::fs::read_to_string(folder.path().join(FILE)).expect("reads");
    let read: serde_json::Value = serde_json::from_str(&text).expect("is json");
    let expected = serde_json::json!({
        "written": 2,
        "series": { key.clone(): { "number": 1, "place": {
            "slice": 5, "window": [40.0, 400.0], "zoom": 2.0, "pan": [3.0, -4.0],
            "flip_h": true, "flip_v": false, "turns": 2,
        } } },
        "studies": { study: { "number": 2, "series": key } },
    });
    assert_eq!(read, expected, "no time, no path, no name: {text}");
}

#[test]
fn past_the_most_that_are_kept_the_ones_written_longest_ago_are_dropped() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let key = |number: usize| format!("{number:016x}");
    let many: Vec<(String, Place)> = (0..MAX_PLACES).map(|n| (key(n), place(n))).collect();
    save(folder.path(), &many, Some((&key(0), &key(0)))).expect("saves");
    assert_eq!(load(folder.path()).series.len(), MAX_PLACES);
    // The first written is written again, so it is no longer the oldest: the second is.
    keep(folder.path(), &key(0), 77);
    keep(folder.path(), "one more", 1);
    let kept = load(folder.path());
    assert_eq!(kept.series.len(), MAX_PLACES);
    assert_eq!(kept.place(&key(0)), Some(place(77)));
    assert_eq!(kept.place(&key(1)), None, "the oldest went");
    assert_eq!(kept.place(&key(2)), Some(place(2)));
    assert_eq!(kept.place("one more"), Some(place(1)));
    // Studies are bounded the same way.
    for study in 1..=MAX_PLACES {
        save(folder.path(), &[], Some((&key(study), "a series"))).expect("saves");
    }
    let kept = load(folder.path());
    assert_eq!(kept.studies.len(), MAX_PLACES);
    assert_eq!(kept.first(&[key(0)]), None, "the oldest study went");
    assert_eq!(kept.first(&[key(1)]), Some("a series"));
}

#[test]
fn a_file_with_far_too_many_entries_is_cut_to_the_most_in_one_write() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let entry = |number: u64| KeptPlace {
        number,
        place: place(0),
    };
    let series = (1..=3000_u64).map(|n| (format!("{n:016x}"), entry(n)));
    let crowded = Kept {
        written: 3000,
        series: series.collect(),
        studies: BTreeMap::new(),
    };
    let text = serde_json::to_string(&crowded).expect("writes");
    std::fs::write(folder.path().join(FILE), text).expect("writes");
    keep(folder.path(), "one more", 1);
    let kept = load(folder.path());
    assert_eq!(kept.series.len(), MAX_PLACES);
    assert!(kept.series.values().all(|entry| entry.number > 2501));
    assert_eq!(kept.place("one more"), Some(place(1)));
}

#[test]
fn a_file_that_cannot_be_read_holds_nothing_and_is_put_aside_not_written_over() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let file = folder.path().join(FILE);
    keep(folder.path(), "one", 1);
    let whole = std::fs::read_to_string(&file).expect("reads");
    let another = whole.replace("\"slice\"", "\"layer\"");
    let cut_short = &whole[..whole.len() / 2];
    let not_text = [0xFF_u8, 0xFE, b'{', 0x00, 0x80];
    let unreadable: [&[u8]; 5] = [
        b"not json",
        another.as_bytes(),
        cut_short.as_bytes(),
        b"",
        &not_text,
    ];
    for bytes in unreadable {
        std::fs::write(&file, bytes).expect("writes");
        assert_eq!(load(folder.path()), Kept::default(), "{bytes:?}");
        assert_eq!(std::fs::read(&file).expect("reads"), bytes, "left alone");
        keep(folder.path(), "two", 2);
        assert_eq!(
            std::fs::read(aside(&file)).expect("reads"),
            bytes,
            "kept aside"
        );
        assert_eq!(load(folder.path()).place("two"), Some(place(2)));
    }
}

#[test]
fn a_number_too_large_to_be_one_is_read_and_then_refused_as_a_place() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let file = folder.path().join(FILE);
    keep(folder.path(), "one", 1);
    let whole = std::fs::read_to_string(&file).expect("reads");
    // More than the numbers of a place can hold: it is read as endless.
    let endless = whole.replace("\"zoom\": 2.0", "\"zoom\": 1e39");
    assert_ne!(endless, whole);
    std::fs::write(&file, endless).expect("writes");
    let read = load(folder.path()).place("one").expect("still a place");
    assert!(read.zoom.is_infinite());
    assert_eq!(read.fitted(30), None);
}
