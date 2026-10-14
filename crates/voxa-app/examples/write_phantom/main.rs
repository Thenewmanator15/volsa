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

use std::path::PathBuf;

mod labels;
mod more;
mod study;

fn main() -> std::io::Result<()> {
    let folder = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("phantom"), PathBuf::from);
    study::write(&folder)?;
    println!("A made-up study was written to {}", folder.display());
    Ok(())
}
