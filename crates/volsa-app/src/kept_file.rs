//! Reading and writing the small files Volsa keeps between runs, one way for all of them: the
//! saved views, the measurements, the preferences, labelling's choices and where each series
//! was left.
//!
//! A file that is missing holds nothing. A file that is there and cannot be read (damaged, cut
//! short, not text, or written by a version this one does not know) holds nothing either, and
//! is left alone until something is to be written in its place: then it is first put aside
//! under its own name with `.unread` after it, so that what it held is not thrown away. A
//! file is written beside itself and then moved over the old one, so that a failure half way
//! leaves the old one whole.

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

/// What reading a kept file came to.
pub(crate) enum Read<T> {
    /// There is no file.
    Missing,
    /// The file, read.
    Held(T),
    /// There is a file, and this Volsa cannot read it.
    NotUnderstood,
}

impl<T> Read<T> {
    /// What the file held, if it was read.
    pub(crate) fn held(self) -> Option<T> {
        match self {
            Self::Held(held) => Some(held),
            Self::Missing | Self::NotUnderstood => None,
        }
    }

    /// Whether there is a file that could not be read, which is to be put aside before
    /// another is written in its place.
    pub(crate) fn is_unread(&self) -> bool {
        matches!(self, Self::NotUnderstood)
    }
}

/// Read a kept file. `understood` says whether what was read is of a version this Volsa knows;
/// a file it says no to is one that is not understood.
pub(crate) fn read<T: DeserializeOwned>(
    file: &Path,
    understood: impl FnOnce(&T) -> bool,
) -> Read<T> {
    let bytes = match std::fs::read(file) {
        Ok(bytes) => bytes,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Read::Missing,
        // Something is there and cannot be read at all: that is not the same as nothing.
        Err(_) => return Read::NotUnderstood,
    };
    // From the bytes, so that a file which is not even text is one that is not understood.
    match serde_json::from_slice::<T>(&bytes) {
        Ok(held) if understood(&held) => Read::Held(held),
        _ => Read::NotUnderstood,
    }
}

/// The name a file that cannot be read is put aside under.
pub(crate) fn aside(file: &Path) -> PathBuf {
    named_after(file, ".unread")
}

/// Write `text` as a kept file, making its folder if it is not there. `over_unread` says that
/// the file now there could not be read: it is put aside first.
///
/// # Errors
///
/// When the folder cannot be made, the file cannot be written, or the one that could not be
/// read cannot be put aside; the file there is then as it was.
pub(crate) fn write(file: &Path, text: &str, over_unread: bool) -> std::io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let beside = named_after(file, ".new");
    std::fs::write(&beside, text)?;
    if over_unread {
        std::fs::rename(file, aside(file))?;
    }
    std::fs::rename(beside, file)
}

/// A file's path with `more` after its name.
fn named_after(file: &Path, more: &str) -> PathBuf {
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(more);
    file.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Made {
        version: u32,
        said: String,
    }

    fn made(version: u32) -> Made {
        Made {
            version,
            said: "kept".to_owned(),
        }
    }

    fn known(made: &Made) -> bool {
        made.version <= 2
    }

    #[test]
    fn a_file_is_missing_or_held_or_not_understood_and_only_the_last_is_put_aside() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let file = folder.path().join("not there yet").join("made.json");
        assert!(matches!(read(&file, known), Read::Missing));
        let text = serde_json::to_string(&made(1)).expect("writes");
        write(&file, &text, false).expect("writes, making the folder");
        assert_eq!(read(&file, known).held(), Some(made(1)));
        assert!(
            !aside(&file).exists(),
            "a file that was read is not put aside"
        );
        // Written again, the old one is replaced, and still nothing is put aside.
        let text = serde_json::to_string(&made(2)).expect("writes");
        write(&file, &text, read(&file, known).is_unread()).expect("writes");
        assert_eq!(read(&file, known).held(), Some(made(2)));
        assert!(!aside(&file).exists());
    }

    #[test]
    fn every_kind_of_file_that_cannot_be_read_is_left_alone_then_put_aside_when_written_over() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let file = folder.path().join("made.json");
        let whole = serde_json::to_string(&made(1)).expect("writes");
        let newer = serde_json::to_string(&made(3)).expect("writes");
        let another = whole.replace("said", "told");
        let cut_short = &whole[..whole.len() / 2];
        let not_text = [0xFF_u8, 0xFE, b'{', 0x00, 0x80];
        let unreadable: [&[u8]; 6] = [
            b"not json",
            newer.as_bytes(),
            another.as_bytes(),
            cut_short.as_bytes(),
            b"",
            &not_text,
        ];
        for bytes in unreadable {
            std::fs::write(&file, bytes).expect("writes");
            let was = read(&file, known);
            assert!(was.is_unread(), "{bytes:?}");
            assert_eq!(std::fs::read(&file).expect("reads"), bytes, "left alone");
            write(&file, &whole, was.is_unread()).expect("writes");
            assert_eq!(
                std::fs::read(aside(&file)).expect("reads"),
                bytes,
                "kept aside"
            );
            assert_eq!(read(&file, known).held(), Some(made(1)));
        }
        assert_eq!(
            aside(&file).file_name().and_then(|name| name.to_str()),
            Some("made.json.unread")
        );
    }
}

#[cfg(test)]
mod every_file_tests {
    //! The rule is one, and every file Volsa keeps follows it.

    use super::aside;
    use crate::annotations::save_annotations;
    use crate::choices::{Choices, save_choices};
    use crate::prefs::{self, Prefs};
    use crate::settings;

    #[test]
    fn each_of_the_five_kept_files_puts_one_it_cannot_read_aside_before_writing_another() {
        type Save = fn(&std::path::Path) -> std::io::Result<()>;
        let kept: [(&str, Save); 5] = [
            ("views.json", |folder| settings::save(folder, "one", &[])),
            ("annotations.json", |folder| {
                save_annotations(folder, "one", &[])
            }),
            ("preferences.json", |folder| {
                prefs::save(folder, &Prefs::default())
            }),
            ("choices.json", |folder| {
                save_choices(folder, &Choices::default())
            }),
            ("places.json", |folder| {
                crate::places::save(folder, &[], None)
            }),
        ];
        for (name, save) in kept {
            let folder = tempfile::tempdir().expect("a temporary folder");
            let file = folder.path().join(name);
            // With no file there, one is written and nothing is put aside.
            save(folder.path()).expect("saves");
            assert!(file.exists() && !aside(&file).exists(), "{name}");
            // And written again over one that was read: still nothing put aside.
            save(folder.path()).expect("saves");
            assert!(!aside(&file).exists(), "{name}");
            // One that cannot be read is put aside, whole, before the next is written.
            std::fs::write(&file, "what another program left here").expect("writes");
            save(folder.path()).expect("saves");
            let put_aside = std::fs::read_to_string(aside(&file)).expect("reads");
            assert_eq!(put_aside, "what another program left here", "{name}");
            assert_ne!(
                std::fs::read_to_string(&file).expect("reads"),
                put_aside,
                "{name}"
            );
        }
    }
}
