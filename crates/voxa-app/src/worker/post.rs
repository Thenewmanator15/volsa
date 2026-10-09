//! Work for the window that no action asks for: done on a worker, with how it went posted as
//! a notice.

use std::sync::Arc;

use super::{Workers, shielded};
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
