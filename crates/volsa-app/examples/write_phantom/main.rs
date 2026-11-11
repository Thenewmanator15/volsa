//! Writes a small made-up study to a folder, for trying Voxa without a real scan.
//!
//! ```text
//! cargo run -p voxa-app --example write_phantom -- some/folder
//! cargo run -p voxa-app -- some/folder
//! ```
//!
//! The study holds two series of a ball that is brighter towards the patient's left, with a
//! small bright bead near the front, so the orientation of the picture can be told at a
//! glance. Beside them are the other kinds of series an export holds: one series holding two
//! volumes of the ball taken at two echo times, three locating images in three planes, and a
//! picture in colour. In `labels` are made-up labels of the ball's parts, as another program
//! might have written them for the finer series:
//!
//! ```text
//! cargo run -p voxa-app -- some/folder --labels some/folder/labels/labels.nii.gz
//! ```
//!
//! With the word `head` it writes instead the made-up head that the model converter makes
//! to check its networks, as a study with a brain's shape, for Voxa to label. It is nobody's
//! scan. The first folder is a model folder's `check` folder; the head is written into the
//! second. The word `whole` after them puts the head in the middle of a grid of air as large
//! as a whole-head scan:
//!
//! ```text
//! cargo run -p voxa-app --example write_phantom -- head some/models/check some/folder [whole]
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod head;
mod labels;
mod more;
mod study;

const USAGE: &str =
    "Usage: write_phantom [FOLDER]\n       write_phantom head CHECK_FOLDER OUT [whole]";

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let written = match args.as_slice() {
        [] => phantom(&PathBuf::from("phantom")),
        [word, check, out, rest @ ..] if word.as_os_str() == "head" => {
            let whole = match rest {
                [] => false,
                [word] if word.as_os_str() == "whole" => true,
                _ => return usage(),
            };
            head::write(check, out, whole).map(|()| out.clone())
        }
        [word, ..] if word.as_os_str() == "head" => return usage(),
        [folder] => phantom(folder),
        _ => return usage(),
    };
    match written {
        Ok(folder) => {
            println!("A made-up study was written to {}", folder.display());
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}

/// Write the phantom into `folder`, and give the folder back.
fn phantom(folder: &Path) -> std::io::Result<PathBuf> {
    study::write(folder)?;
    Ok(folder.to_owned())
}
