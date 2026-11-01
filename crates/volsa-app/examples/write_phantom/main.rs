//! Writes a small made-up study to a folder, for trying Volsa without a real scan.
//!
//! ```text
//! cargo run -p volsa-app --example write_phantom -- some/folder
//! cargo run -p volsa-app -- some/folder
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
//! cargo run -p volsa-app -- some/folder --labels some/folder/labels/labels.nii.gz
//! ```
//!
//! With the word `head` it writes instead the made-up head that the model converter makes
//! to check its networks, as a study with a brain's shape, for Volsa to label. It is nobody's
//! scan. The first folder is a model folder's `check` folder; the head is written into the
//! second. The word `whole` after them puts the head in the middle of a grid of air as large
//! as a whole-head scan:
//!
//! ```text
//! cargo run -p volsa-app --example write_phantom -- head some/models/check some/folder [whole]
//! ```
//!
//! For measuring Volsa on scans as large as CT and whole-body MRI, the words `ct` and `body`
//! write a made-up body from the thighs to the head: `ct` as one CT series of thin slices
//! (512 x 512, 2,000 slices unless a number is given), `body` as a whole-body MR study taken
//! in overlapping stations (five unless a number is given), each a series, beside the
//! stations joined into one. `zip` puts a folder into a zip, compressed unless `stored`:
//!
//! ```text
//! cargo run --release -p volsa-app --example write_phantom -- ct some/ct [SLICES]
//! cargo run --release -p volsa-app --example write_phantom -- body some/body [STATIONS]
//! cargo run --release -p volsa-app --example write_phantom -- zip some/ct some/ct.zip [stored]
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod big;
mod body;
mod head;
mod labels;
mod more;
mod study;

const USAGE: &str = "Usage: write_phantom [FOLDER]
       write_phantom head CHECK_FOLDER OUT [whole]
       write_phantom ct OUT [SLICES]
       write_phantom body OUT [STATIONS]
       write_phantom zip FOLDER OUT [stored]";

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
        [word, rest @ ..] if is_large(word) => match large(word, rest) {
            Some(written) => written,
            None => return usage(),
        },
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

fn is_large(word: &Path) -> bool {
    ["ct", "body", "zip"]
        .iter()
        .any(|large| word.as_os_str() == *large)
}

/// Write a large study, or a zip of one, as the words after `word` say; `None` when they do
/// not say it right.
fn large(word: &Path, rest: &[PathBuf]) -> Option<std::io::Result<PathBuf>> {
    let number = |given: Option<&PathBuf>, otherwise: u32| match given {
        None => Some(otherwise),
        Some(given) => given.to_str()?.parse().ok().filter(|n| *n > 0),
    };
    let written = match (word.to_str()?, rest) {
        ("ct", [out, more @ ..]) if more.len() < 2 => {
            big::ct(out, number(more.first(), 2000)?).map(|()| out.clone())
        }
        ("body", [out, more @ ..]) if more.len() < 2 => {
            big::whole_body(out, number(more.first(), 5)?).map(|()| out.clone())
        }
        ("zip", [folder, out]) => big::zip(folder, out, false).map(|()| out.clone()),
        ("zip", [folder, out, how]) if how.as_os_str() == "stored" => {
            big::zip(folder, out, true).map(|()| out.clone())
        }
        _ => return None,
    };
    Some(written)
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
