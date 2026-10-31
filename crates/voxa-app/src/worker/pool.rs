//! The threads background work runs on.

use voxa_dicom::READING_STACK_BYTES;

/// A pool of threads named `{name}-0`, `{name}-1` and so on, one for each core, each with the
/// stack a reader of DICOM files is given; `None` when the system would not make them.
///
/// Every file is read on these threads, never on the window's: its stack is smaller still,
/// and a thread that runs out of stack stops the whole program.
pub(super) fn reading(name: &'static str) -> Option<rayon::ThreadPool> {
    rayon::ThreadPoolBuilder::new()
        .thread_name(move |n| format!("{name}-{n}"))
        .stack_size(READING_STACK_BYTES)
        .build()
        .ok()
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::super::tests::Bench;
    use crate::action::Action;
    use crate::state::{Notice, NoticeKind};

    fn notice(text: &str) -> Notice {
        Notice {
            kind: NoticeKind::Info,
            text: text.to_owned(),
            details: Vec::new(),
            key: Some("stack"),
        }
    }

    /// Use at least `bytes` of the stack below `top`, the address of something on it.
    fn use_stack(top: usize, bytes: usize) -> u8 {
        let room = black_box([1u8; 1024]);
        let here = std::ptr::addr_of!(room).addr();
        if top.saturating_sub(here) >= bytes {
            return room[0];
        }
        // Not a call in tail position, so each level keeps its own room.
        use_stack(top, bytes).wrapping_add(room[1023])
    }

    #[test]
    fn work_runs_on_threads_with_room_for_eight_mebibytes_of_stack() {
        let bench = Bench::with_budget(1 << 20);
        bench.workers.post(notice("stopped"), || {
            let top = 0u8;
            let top = std::ptr::addr_of!(top).addr();
            black_box(use_stack(top, 8 * 1024 * 1024));
            notice("done")
        });
        assert_eq!(bench.next(), Action::Post(notice("done")));
    }
}
