//! What Volsa keeps between one run and the next: the saved views of each series, in one small
//! file in the folder Volsa keeps on this computer alone (see [`crate::moved`]). The file holds
//! a hash for each series and the views saved for it, and nothing that says whose scan it
//! was. The folder the system gives for settings holds only the preferences.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::views::SavedView;

/// The name of the file within its folder.
const FILE: &str = "views.json";
/// The name a file Volsa cannot read is given before a new one is written in its place, so
/// that what was damaged, or written by another version, is not thrown away.
const SET_ASIDE: &str = "views.json.unread";

/// Every series' views, under each series' key.
type Kept = BTreeMap<String, Vec<SavedView>>;

/// The folder the system gives Volsa for its settings, which on Windows roams with the
/// person's profile: it holds only the preferences. `None` when the system has no such place,
/// in which case they are not kept between runs.
pub fn folder() -> Option<PathBuf> {
    let places = directories::ProjectDirs::from("", "", "Volsa")?;
    Some(places.config_dir().to_owned())
}

/// What reading the file came to.
enum Read {
    /// There is no file.
    Missing,
    /// The file, read.
    Held(Kept),
    /// There is a file, and this Volsa cannot read it: it is damaged, cut short, or not what
    /// this version wrote.
    NotUnderstood,
}

fn read(folder: &Path) -> Read {
    let bytes = match std::fs::read(folder.join(FILE)) {
        Ok(bytes) => bytes,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Read::Missing,
        // Something is there and cannot be read at all: that is not the same as nothing.
        Err(_) => return Read::NotUnderstood,
    };
    serde_json::from_slice(&bytes).map_or(Read::NotUnderstood, Read::Held)
}

/// The views saved for the series with this key. A file that is missing, damaged, cut short
/// or not what this version wrote is read as holding none.
pub fn load(folder: &Path, key: &str) -> Vec<SavedView> {
    match read(folder) {
        Read::Held(mut kept) => kept.remove(key).unwrap_or_default(),
        Read::Missing | Read::NotUnderstood => Vec::new(),
    }
}

/// Keep these views for the series with this key, in place of any kept for it before; the
/// views of other series are left as they are. The folder is made if it is not there. A file
/// already there that cannot be read is first put aside under another name, so that the views
/// of every other series in it are not thrown away.
///
/// # Errors
///
/// When the folder cannot be made or the file cannot be written.
pub fn save(folder: &Path, key: &str, views: &[SavedView]) -> std::io::Result<()> {
    let (file, aside) = (folder.join(FILE), folder.join(SET_ASIDE));
    let (mut all, unread) = match read(folder) {
        Read::Held(kept) => (kept, false),
        Read::Missing => (Kept::new(), false),
        Read::NotUnderstood => (Kept::new(), true),
    };
    if views.is_empty() {
        all.remove(key);
    } else {
        all.insert(key.to_owned(), views.to_vec());
    }
    let text = serde_json::to_string_pretty(&all)?;
    std::fs::create_dir_all(folder)?;
    // Written beside the file and then moved over it, so a failure half way leaves the old one.
    let beside = folder.join(format!("{FILE}.new"));
    std::fs::write(&beside, text)?;
    if unread {
        std::fs::rename(&file, aside)?;
    }
    std::fs::rename(beside, file)
}

#[cfg(test)]
mod tests {
    use volsa_core::WindowLevel;

    use super::*;
    use crate::scene::Scene;

    fn view(name: &str) -> SavedView {
        SavedView::of(name, 1, &Scene::default(), WindowLevel::new(40.0, 400.0))
    }

    #[test]
    fn views_that_are_saved_are_loaded_again() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let views = vec![view("Front"), view("Ear")];
        save(folder.path(), "0123456789abcdef", &views).expect("saves");
        assert_eq!(load(folder.path(), "0123456789abcdef"), views);
        assert_eq!(load(folder.path(), "another series"), Vec::new());
    }

    #[test]
    fn the_views_of_two_series_share_a_file_and_do_not_disturb_each_other() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        save(folder.path(), "one", &[view("A")]).expect("saves");
        save(folder.path(), "two", &[view("B"), view("C")]).expect("saves");
        save(folder.path(), "one", &[view("A"), view("D")]).expect("saves");
        assert_eq!(load(folder.path(), "one"), [view("A"), view("D")]);
        assert_eq!(load(folder.path(), "two"), [view("B"), view("C")]);
        save(folder.path(), "two", &[]).expect("saves");
        assert_eq!(
            load(folder.path(), "two"),
            Vec::new(),
            "the last one deleted"
        );
        assert_eq!(load(folder.path(), "one").len(), 2);
    }

    #[test]
    fn a_folder_that_is_not_there_is_made() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let deeper = folder.path().join("not").join("there yet");
        save(&deeper, "one", &[view("A")]).expect("saves");
        assert_eq!(load(&deeper, "one"), [view("A")]);
        assert_eq!(load(&folder.path().join("never made"), "one"), Vec::new());
    }

    #[test]
    fn a_file_that_cannot_be_read_holds_nothing_and_is_put_aside_not_written_over() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let (file, aside) = (folder.path().join(FILE), folder.path().join(SET_ASIDE));
        save(folder.path(), "one", &[view("A")]).expect("saves");
        let whole = std::fs::read_to_string(&file).expect("reads");
        // One view that says less than a view must, as another version's might.
        let another = whole.replace("\"name\"", "\"called\"");
        let cut_short = &whole[..whole.len() / 2];
        for unreadable in ["not json", another.as_str(), cut_short, ""] {
            std::fs::write(&file, unreadable).expect("writes");
            assert_eq!(load(folder.path(), "one"), Vec::new(), "{unreadable}");
            assert_eq!(
                std::fs::read_to_string(&file).expect("reads"),
                unreadable,
                "left alone"
            );
            save(folder.path(), "two", &[view("B")]).expect("saves");
            assert_eq!(
                std::fs::read_to_string(&aside).expect("reads"),
                unreadable,
                "kept aside"
            );
            assert_eq!(load(folder.path(), "two"), [view("B")]);
        }
    }

    #[test]
    fn a_file_that_is_not_even_text_is_put_aside_too() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let (file, aside) = (folder.path().join(FILE), folder.path().join(SET_ASIDE));
        let damaged = [0xFF_u8, 0xFE, b'{', 0x00, 0x80];
        std::fs::write(&file, damaged).expect("writes");
        assert_eq!(load(folder.path(), "one"), Vec::new());
        save(folder.path(), "one", &[view("A")]).expect("saves");
        assert_eq!(std::fs::read(&aside).expect("reads"), damaged, "kept aside");
        assert_eq!(load(folder.path(), "one"), [view("A")]);
    }

    #[test]
    fn the_file_is_plain_json_that_another_program_can_read() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        save(folder.path(), "one", &[view("Front")]).expect("saves");
        let text = std::fs::read_to_string(folder.path().join(FILE)).expect("reads");
        let read: serde_json::Value = serde_json::from_str(&text).expect("is json");
        assert_eq!(read["one"][0]["name"], "Front");
        assert_eq!(read["one"][0]["look"], "Surface");
        assert_eq!(read["one"][0]["window"][1], 400.0);
    }
}
