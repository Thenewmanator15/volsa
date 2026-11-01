//! What Volsa keeps between one run and the next: the saved views of each series, in one small
//! file in the folder Volsa keeps on this computer alone (see [`crate::moved`]). The file holds
//! a hash for each series and the views saved for it, and nothing that says whose scan it
//! was. The folder the system gives for settings holds only the preferences.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::views::SavedView;

/// The name of the file within its folder.
const FILE: &str = "views.json";

/// Every series' views, under each series' key.
type Kept = BTreeMap<String, Vec<SavedView>>;

/// The folder the system gives Volsa for its settings, which on Windows roams with the
/// person's profile: it holds only the preferences. `None` when the system has no such place,
/// in which case they are not kept between runs.
pub fn folder() -> Option<PathBuf> {
    let places = directories::ProjectDirs::from("", "", "Volsa")?;
    Some(places.config_dir().to_owned())
}

/// Everything in the file. A file that is missing, cannot be read or is not what Volsa wrote
/// is read as holding nothing.
fn kept(folder: &Path) -> Kept {
    let text = std::fs::read_to_string(folder.join(FILE)).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

/// The views saved for the series with this key.
pub fn load(folder: &Path, key: &str) -> Vec<SavedView> {
    kept(folder).remove(key).unwrap_or_default()
}

/// Keep these views for the series with this key, in place of any kept for it before; the
/// views of other series are left as they are. The folder is made if it is not there.
///
/// # Errors
///
/// When the folder cannot be made or the file cannot be written.
pub fn save(folder: &Path, key: &str, views: &[SavedView]) -> std::io::Result<()> {
    let mut all = kept(folder);
    if views.is_empty() {
        all.remove(key);
    } else {
        all.insert(key.to_owned(), views.to_vec());
    }
    let text = serde_json::to_string_pretty(&all)?;
    std::fs::create_dir_all(folder)?;
    // Written beside the file and then moved over it, so a failure half way leaves the old one.
    let (file, beside) = (folder.join(FILE), folder.join(format!("{FILE}.new")));
    std::fs::write(&beside, text)?;
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
    fn a_damaged_file_is_read_as_empty_and_can_be_written_over() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        std::fs::write(folder.path().join(FILE), "not json").expect("writes");
        assert_eq!(load(folder.path(), "one"), Vec::new());
        save(folder.path(), "one", &[view("A")]).expect("saves");
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
