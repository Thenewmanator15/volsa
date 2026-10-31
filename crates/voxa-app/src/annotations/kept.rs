//! Annotations kept between one run and the next, in one small file beside the saved views.
//! The file holds a hash for each series and that series' annotations: coordinates, numbers
//! and the names the person typed. Nothing in it says whose scan it was.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::Annotation;

/// The version of the file this Voxa writes, and the newest it reads. Version 2 says for each
/// annotation whose millimetres it is in; version 1, which 0.6.0 wrote, says nothing of that,
/// and is read as it was made, in a 3D view, in the patient's.
pub const ANNOTATIONS_VERSION: u32 = 2;
/// The name of the file within its folder, beside the saved views.
const FILE: &str = "annotations.json";
/// The name a file Voxa cannot read is given before a new one is written in its place, so
/// that what a newer Voxa wrote, or what was damaged, is not thrown away.
const SET_ASIDE: &str = "annotations.json.unread";

/// What the file holds: its version, and every series' annotations under that series' key.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Kept {
    version: u32,
    series: BTreeMap<String, Vec<Annotation>>,
}

/// What reading the file came to.
enum Read {
    /// There is no file.
    Missing,
    /// The file, read.
    Held(Kept),
    /// There is a file, and this Voxa cannot read it: it is damaged, cut short, or written
    /// by a newer version.
    NotUnderstood,
}

fn read(folder: &Path) -> Read {
    let bytes = match std::fs::read(folder.join(FILE)) {
        Ok(bytes) => bytes,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Read::Missing,
        // Something is there and cannot be read at all: that is not the same as nothing.
        Err(_) => return Read::NotUnderstood,
    };
    // From the bytes, so that a file which is not even text is one that is not understood.
    match serde_json::from_slice::<Kept>(&bytes) {
        Ok(kept) if kept.version <= ANNOTATIONS_VERSION => Read::Held(kept),
        _ => Read::NotUnderstood,
    }
}

/// The annotations kept for the series with this key. A file that is missing, damaged, cut
/// short or of a later version is read as holding none.
pub fn load_annotations(folder: &Path, key: &str) -> Vec<Annotation> {
    match read(folder) {
        Read::Held(mut kept) => kept.series.remove(key).unwrap_or_default(),
        Read::Missing | Read::NotUnderstood => Vec::new(),
    }
}

/// Keep these annotations for the series with this key, in place of any kept for it before;
/// those of other series are left as they are. The folder is made if it is not there. A file
/// already there that cannot be read is first put aside under another name.
///
/// # Errors
///
/// When the folder cannot be made or the file cannot be written.
pub fn save_annotations(folder: &Path, key: &str, items: &[Annotation]) -> std::io::Result<()> {
    let (file, aside) = (folder.join(FILE), folder.join(SET_ASIDE));
    let mut all = match read(folder) {
        Read::Held(kept) => kept,
        Read::Missing => Kept::default(),
        Read::NotUnderstood => {
            std::fs::rename(&file, aside)?;
            Kept::default()
        }
    };
    all.version = ANNOTATIONS_VERSION;
    if items.is_empty() {
        all.series.remove(key);
    } else {
        all.series.insert(key.to_owned(), items.to_vec());
    }
    let text = serde_json::to_string_pretty(&all)?;
    std::fs::create_dir_all(folder)?;
    // Written beside the file and then moved over it, so a failure half way leaves the old one.
    let beside = folder.join(format!("{FILE}.new"));
    std::fs::write(&beside, text)?;
    std::fs::rename(beside, file)
}

#[cfg(test)]
mod tests {
    use voxa_volume::RegionStats;

    use super::super::Shape;
    use super::*;
    use crate::measuring::Scale;

    fn marker(id: u64, label: &str) -> Annotation {
        let shape = Shape::Marker {
            at: [1.0, 2.0, 3.0],
        };
        Annotation {
            id,
            label: label.to_owned(),
            shape,
            scale: Scale::Patient,
        }
    }

    fn region(id: u64) -> Annotation {
        let stats = RegionStats {
            mean: 312.5,
            sd: 41.25,
            min: 201.0,
            max: 388.0,
            area_mm2: 78.5,
            samples: 412,
        };
        let shape = Shape::Region {
            centre: [0.5, -2.0, 30.0],
            across: [1.0, 0.0, 0.0],
            down: [0.0, 1.0, 0.0],
            radii_mm: [5.0, 2.5],
            stats: Some(stats),
        };
        Annotation {
            id,
            label: "Region 2".to_owned(),
            shape,
            // Made on a picture placed by its pixel spacing alone, which is kept with it.
            scale: Scale::PixelSpacing,
        }
    }

    #[test]
    fn annotations_that_are_kept_are_read_again_with_everything_they_held() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let items = vec![marker(1, "the tip"), region(2)];
        save_annotations(folder.path(), "0123456789abcdef", &items).expect("saves");
        assert_eq!(load_annotations(folder.path(), "0123456789abcdef"), items);
        assert_eq!(load_annotations(folder.path(), "another series"), []);
        let text = std::fs::read_to_string(folder.path().join(FILE)).expect("reads");
        assert!(text.contains("\"version\": 2"), "{text}");
    }

    #[test]
    fn the_annotations_of_two_series_share_a_file_and_do_not_disturb_each_other() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        save_annotations(folder.path(), "one", &[marker(1, "A")]).expect("saves");
        save_annotations(folder.path(), "two", &[marker(1, "B"), region(2)]).expect("saves");
        save_annotations(folder.path(), "one", &[marker(1, "A"), marker(2, "D")]).expect("saves");
        assert_eq!(
            load_annotations(folder.path(), "one"),
            [marker(1, "A"), marker(2, "D")]
        );
        assert_eq!(
            load_annotations(folder.path(), "two"),
            [marker(1, "B"), region(2)]
        );
        save_annotations(folder.path(), "two", &[]).expect("saves");
        assert_eq!(
            load_annotations(folder.path(), "two"),
            [],
            "the last one removed"
        );
        assert_eq!(load_annotations(folder.path(), "one").len(), 2);
    }

    #[test]
    fn a_folder_that_is_not_there_is_made() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let deeper = folder.path().join("not").join("there yet");
        save_annotations(&deeper, "one", &[marker(1, "A")]).expect("saves");
        assert_eq!(load_annotations(&deeper, "one"), [marker(1, "A")]);
        assert_eq!(
            load_annotations(&folder.path().join("never made"), "one"),
            []
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_holds_nothing_and_is_put_aside_not_thrown_away() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let (file, aside) = (folder.path().join(FILE), folder.path().join(SET_ASIDE));
        save_annotations(folder.path(), "one", &[marker(1, "A")]).expect("saves");
        let whole = std::fs::read_to_string(&file).expect("reads");
        let newer = whole.replace("\"version\": 2", "\"version\": 3");
        let cut_short = &whole[..whole.len() / 2];
        for unreadable in ["not json", newer.as_str(), cut_short, ""] {
            std::fs::write(&file, unreadable).expect("writes");
            assert_eq!(load_annotations(folder.path(), "one"), [], "{unreadable}");
            assert_eq!(
                std::fs::read_to_string(&file).expect("reads"),
                unreadable,
                "left alone"
            );
            save_annotations(folder.path(), "two", &[marker(1, "B")]).expect("saves");
            assert_eq!(
                std::fs::read_to_string(&aside).expect("reads"),
                unreadable,
                "kept aside"
            );
            assert_eq!(load_annotations(folder.path(), "two"), [marker(1, "B")]);
        }
    }

    #[test]
    fn a_file_that_is_not_even_text_is_put_aside_too() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let (file, aside) = (folder.path().join(FILE), folder.path().join(SET_ASIDE));
        let damaged = [0xFF_u8, 0xFE, b'{', 0x00, 0x80];
        std::fs::write(&file, damaged).expect("writes");
        assert_eq!(load_annotations(folder.path(), "one"), []);
        assert_eq!(std::fs::read(&file).expect("reads"), damaged, "left alone");
        save_annotations(folder.path(), "one", &[marker(1, "A")]).expect("saves");
        assert_eq!(std::fs::read(&aside).expect("reads"), damaged, "kept aside");
        assert_eq!(load_annotations(folder.path(), "one"), [marker(1, "A")]);
    }
}
