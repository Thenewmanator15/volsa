//! Work for the window that no action asks for: done on a worker, with how it went posted as
//! a notice.

use std::path::Path;
use std::sync::Arc;

use voxa_dicom::Index;

use super::{Shared, Workers, shielded};
use crate::action::Action;
use crate::state::Notice;

impl Workers {
    /// Do a piece of work off the interface's thread, and post what it says of how it went.
    /// If the work stops part-way, `otherwise` is posted in its place: something is always
    /// said.
    pub fn post(&self, otherwise: Notice, work: impl FnOnce() -> Notice + Send + 'static) {
        let shared = Arc::clone(&self.shared);
        self.spawn(move || {
            let mut said = otherwise;
            shielded(|| said = work());
            shared.report(Action::Post(said));
        });
    }
}

impl Shared {
    /// Say that an export could not be opened, and log what kind of failure it was: the
    /// reason goes to the window in full, where it may name the file, and to the log as its
    /// kind alone.
    pub(super) fn could_not_open(&self, error: &voxa_core::VoxaError) {
        tracing::warn!("an export could not be opened: {}", error.kind());
        self.report(Action::IndexFailed(error.to_string()));
    }
}

/// What to say when a source opened but held no images.
pub(super) fn nothing_found(path: &Path, found: &Index) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let name = name.unwrap_or_else(|| path.display().to_string());
    // A lone file that is not a zip is read as a source of that one file.
    if path.is_file() && found.skipped == [name.clone()] {
        "That file is not a zip or DICOM export".to_owned()
    } else {
        format!("No images were found in {name}")
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::Bench;
    use crate::action::Action;
    use crate::state::{Notice, NoticeKind};

    fn notice(text: &str) -> Notice {
        Notice {
            kind: NoticeKind::Info,
            text: text.to_owned(),
            details: Vec::new(),
            key: Some("posting"),
        }
    }

    #[test]
    fn work_done_on_a_worker_posts_how_it_went() {
        let bench = Bench::with_budget(1 << 20);
        let asking = std::thread::current().id();
        bench.workers.post(notice("stopped"), move || {
            let here = std::thread::current().id();
            notice(if here == asking {
                "done on the thread that asked"
            } else {
                "done"
            })
        });
        assert_eq!(bench.next(), Action::Post(notice("done")));
        assert!(bench.is_quiet(), "and it is said once");
    }

    #[test]
    fn work_that_stops_part_way_still_has_something_said_of_it() {
        let bench = Bench::with_budget(1 << 20);
        bench
            .workers
            .post(notice("stopped"), || panic!("the encoder gave up"));
        assert_eq!(bench.next(), Action::Post(notice("stopped")));
        assert!(bench.is_quiet());
    }
}
