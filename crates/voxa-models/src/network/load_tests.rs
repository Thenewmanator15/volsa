//! What Voxa says when the runtime's library would not load, sorted from what Windows said.
//! Made-up errors only: no library is loaded.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use super::{not_loaded, wants_visual_cpp};
use crate::runtime::RuntimeError;

/// An error as the library loader gives one: its own words, with Windows' beneath them.
#[derive(Debug)]
struct Failed(io::Error);

impl fmt::Display for Failed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LoadLibraryExW failed")
    }
}

impl Error for Failed {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

/// What Windows says for its error `code`, beneath the loader's words.
fn windows_said(code: i32) -> Failed {
    Failed(io::Error::from_raw_os_error(code))
}

#[test]
fn a_library_the_runtime_needs_that_is_missing_or_too_old_is_put_down_to_visual_cpp() {
    // A module that could not be found, a procedure not found in one, and a library whose start
    // failed: what an absent or old Visual C++ redistributable makes Windows say.
    for code in [126, 127, 1114] {
        assert!(wants_visual_cpp(&windows_said(code)), "{code}");
    }
    // A file that is no program, a refusal, and the same numbers anywhere but as Windows' code.
    for code in [193, 5, 12, 1260] {
        assert!(!wants_visual_cpp(&windows_said(code)), "{code}");
    }
    let near = io::Error::other("failed (os error 126 and more) or (os error 1260)");
    assert!(!wants_visual_cpp(&near));
    // Windows' own words, at the top as well as beneath.
    assert!(wants_visual_cpp(&io::Error::from_raw_os_error(126)));
}

#[test]
fn the_runtime_put_down_to_visual_cpp_says_which_redistributable_it_needs() {
    let said = RuntimeError::NeedsVisualCpp.to_string();
    assert_eq!(
        said,
        "The runtime in that folder would not load, most likely because this computer lacks the \
         Microsoft Visual C++ 2015-2022 x64 redistributable, or has an old one. ONNX Runtime \
         needs it, and Microsoft gives it free."
    );
}

#[test]
fn a_runtime_older_than_the_interface_asked_for_is_the_wrong_build_and_others_would_not_load() {
    let path = PathBuf::from("made up");
    let said = |version: &str| ort::LoadDynamicError::BadVersion {
        version_str: version.to_owned(),
        path: path.clone(),
    };
    assert_eq!(not_loaded(&said("1.16.3")), RuntimeError::WrongBuild);
    assert_eq!(not_loaded(&said("1.24.4")), RuntimeError::WouldNotLoad);
    let no_interface = ort::LoadDynamicError::MissingApi { path };
    assert_eq!(not_loaded(&no_interface), RuntimeError::WouldNotLoad);
}
