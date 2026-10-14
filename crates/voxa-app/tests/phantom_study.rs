//! The made-up study that `write_phantom` writes holds one series of each kind Voxa tells
//! apart, and none of them at fault.

use voxa_core::SeriesKind;
use voxa_dicom::{index, open_source};

#[path = "../examples/write_phantom/labels.rs"]
mod labels;
#[path = "../examples/write_phantom/more.rs"]
mod more;
#[path = "../examples/write_phantom/study.rs"]
mod study;

#[test]
fn the_made_up_study_holds_one_series_of_each_kind_and_none_at_fault() {
    let folder = tempfile::tempdir().expect("a folder to write into");
    study::write(folder.path()).expect("the study is written");
    let mut source = open_source(folder.path()).expect("the folder opens");
    let found = index(&mut *source, &mut |_| {}).expect("the folder is indexed");
    let listed: Vec<(&str, SeriesKind, Option<&str>)> = found
        .series
        .iter()
        .map(|series| {
            let info = &series.info;
            let set = info.set.as_ref().map(|set| set.what.as_str());
            (info.description.as_str(), info.kind(), set)
        })
        .collect();
    let expected = [
        ("ball, coarse", SeriesKind::Volume, None),
        ("ball, fine", SeriesKind::Volume, None),
        ("ball, two echoes", SeriesKind::Volume, Some("echo 20 ms")),
        ("ball, two echoes", SeriesKind::Volume, Some("echo 80 ms")),
        ("locating", SeriesKind::Locating, None),
        ("a picture in colour", SeriesKind::Colour, None),
    ];
    assert_eq!(listed, expected);
    // Beside the images are only the made-up labels and their table.
    assert_eq!(found.skipped.len(), 2, "{:?}", found.skipped);
}
