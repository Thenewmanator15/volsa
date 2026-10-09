//! Voxa: a viewer for MRI scans. Not a medical device; not for diagnostic use.

use std::path::PathBuf;

fn main() -> eframe::Result {
    let initial = std::env::args_os().nth(1).map(PathBuf::from);
    voxa_app::run(initial)
}
