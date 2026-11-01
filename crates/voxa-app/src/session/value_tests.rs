//! Reading one value of a slice in place, as the pointer moves over a picture: no slice is
//! built or copied to read it.

use std::sync::Arc;

use voxa_dicom::Frame;

use super::SLICES_COPIED;
use super::tests::{open_until, ready_in_3d, session, zip_of};
use super::*;
use crate::testing::{FINE_SCAN, id, three_series_files};

/// How many slices `cached` has copied out of a volume on this thread so far.
fn copied() -> usize {
    SLICES_COPIED.with(std::cell::Cell::get)
}

/// A session with the 64-slice series shown in 3D, so that its volume is the one kept, and
/// none of its slices in the frame cache.
fn session_holding_a_volume_and_no_slices() -> Session {
    let mut session = session();
    let (_folder, path) = zip_of(&three_series_files());
    open_until(&mut session, &path, |session| {
        ready_in_3d(session, FINE_SCAN)
    });
    let mut cache = session.cache().lock().expect("no worker panicked");
    cache.forget(&id(FINE_SCAN));
    drop(cache);
    session
}

#[test]
fn a_kept_volume_gives_a_value_with_the_frame_cache_empty_and_no_slice_built() {
    let session = session_holding_a_volume_and_no_slices();
    let series = id(FINE_SCAN);
    let volume = session.volume(&series).expect("kept");
    let held = || session.cache().lock().expect("no worker panicked").bytes();
    let (before, held_before) = (copied(), held());
    for slice in [0_u32, 31, 63] {
        let read = session.value_at(&series, slice as usize, (3, 5));
        assert_eq!(read, volume.value(3, 5, slice), "slice {slice}");
        assert!(read.is_some());
    }
    assert_eq!(copied(), before, "no slice was copied out of the volume");
    assert_eq!(held(), held_before, "and none was put in the cache");
}

#[test]
fn asking_for_a_whole_slice_does_copy_which_is_what_the_value_must_not_do() {
    // The counter above counts what it says it counts.
    let session = session_holding_a_volume_and_no_slices();
    let before = copied();
    let frame = session.cached(&id(FINE_SCAN), 31).expect("read");
    assert_eq!((frame.cols, frame.rows), (8, 8));
    assert_eq!(copied(), before + 1);
}

#[test]
fn a_value_off_the_slice_or_of_a_slice_that_is_not_there_is_none() {
    let session = session_holding_a_volume_and_no_slices();
    let series = id(FINE_SCAN);
    assert_eq!(
        session.value_at(&series, 0, (8, 0)),
        None,
        "past the columns"
    );
    assert_eq!(session.value_at(&series, 0, (0, 8)), None, "past the rows");
    assert_eq!(session.value_at(&series, 64, (0, 0)), None, "no such slice");
    assert_eq!(session.value_at(&series, usize::MAX, (0, 0)), None);
    assert_eq!(session.value_at(&series, 0, (usize::MAX, usize::MAX)), None);
    let other = id("9.9.9");
    assert_eq!(session.value_at(&other, 0, (0, 0)), None, "another series");
}

/// A 3 by 2 slice whose values are their places, 0 to 5, in colour or not.
fn slice_of_places(colour: bool) -> Arc<Frame> {
    let values = (0..6_u8).map(f32::from).collect();
    let colour = colour.then(|| vec![[9, 9, 9]; 6]);
    Arc::new(Frame {
        rows: 2,
        cols: 3,
        values,
        colour,
    })
}

#[test]
fn a_decoded_slice_is_read_in_place_and_a_picture_in_colour_has_no_value() {
    let session = session();
    let (grey, coloured) = (id("1.1"), id("1.2"));
    {
        let mut cache = session.cache().lock().expect("no worker panicked");
        cache.insert(&grey, 4, slice_of_places(false), 4);
        cache.insert(&coloured, 4, slice_of_places(true), 4);
    }
    let before = copied();
    assert_eq!(session.value_at(&grey, 4, (2, 1)), Some(5.0));
    assert_eq!(session.value_at(&grey, 4, (0, 1)), Some(3.0));
    assert_eq!(session.value_at(&grey, 4, (3, 0)), None, "past the columns");
    assert_eq!(session.value_at(&grey, 4, (0, 2)), None, "past the rows");
    assert_eq!(session.value_at(&grey, 5, (0, 0)), None, "not decoded");
    assert_eq!(
        session.value_at(&coloured, 4, (2, 1)),
        None,
        "its values only say how bright it is"
    );
    assert_eq!(copied(), before);
}

#[test]
fn a_slice_is_read_where_it_lies_whether_decoded_or_in_a_kept_volume() {
    let session = session_holding_a_volume_and_no_slices();
    let series = id(FINE_SCAN);
    let volume = session.volume(&series).expect("kept");
    let before = copied();
    // The kept volume holds its values as stored: they are turned into values to be read,
    // and nothing is copied into the frame cache.
    let read = session.read_slice(&series, 31, |values| (values.values.to_vec(), values.size));
    let kept = volume.slice_values(31).expect("in the volume");
    assert_eq!(read, Some((kept, [8, 8])), "the volume's own values");
    let decoded = slice_of_places(false);
    let at = decoded.values.as_ptr();
    let grey = id("1.1");
    let mut cache = session.cache().lock().expect("no worker panicked");
    cache.insert(&grey, 2, decoded, 4);
    drop(cache);
    let read = session.read_slice(&grey, 2, |values| (values.values.as_ptr(), values.size));
    assert_eq!(read, Some((at, [3, 2])), "the decoded slice's own values");
    assert_eq!(copied(), before, "and nothing was copied");
    assert_eq!(session.read_slice(&grey, 3, |_| ()), None, "not decoded");
}
