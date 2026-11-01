//! The one file of many frames opened last, kept so that asking for more of its frames does
//! not read and parse it all over again.

use std::sync::{Arc, Mutex};

use voxa_core::VoxaError;
use voxa_dicom::{ImageFile, IndexedSeries};

use super::{Batch, Request, Shared, lock};

/// Holds one opened file. A file that holds a whole series can be hundreds of megabytes:
/// one is kept, and it is let go of when another is asked for or another export is opened.
#[derive(Default)]
pub(super) struct Opened {
    held: Mutex<Option<Held>>,
}

struct Held {
    /// The name of the entry the file was read from.
    entry: String,
    /// Which export it came from, as the worker numbers them.
    export: u64,
    file: Arc<ImageFile>,
}

impl Opened {
    /// The file stored as `entry` in export number `export`: the one held, when it is that
    /// one, and otherwise opened with `open` and held in its place. Whoever asks for a file
    /// while it is being opened waits and then shares it.
    ///
    /// # Errors
    ///
    /// Whatever `open` gives. A file that fails to open is not held.
    pub(super) fn get(
        &self,
        entry: &str,
        export: u64,
        open: impl FnOnce() -> Result<ImageFile, VoxaError>,
    ) -> Result<Arc<ImageFile>, VoxaError> {
        let mut held = lock(&self.held);
        let is_it = |held: &&Held| held.entry == entry && held.export == export;
        if let Some(found) = held.as_ref().filter(is_it) {
            return Ok(Arc::clone(&found.file));
        }
        // Let go of before the next is opened, so that two such files are never held at once.
        *held = None;
        let file = Arc::new(open()?);
        *held = Some(Held {
            entry: entry.to_owned(),
            export,
            file: Arc::clone(&file),
        });
        Ok(file)
    }

    /// Let go of the file held.
    pub(super) fn clear(&self) {
        *lock(&self.held) = None;
    }
}

/// Group slices by the file that holds them, keeping the order they were asked for in: a
/// file is read and parsed once however many of its frames are wanted.
pub(super) fn batches(listed: &IndexedSeries, wanted: Vec<usize>) -> Vec<Batch> {
    let mut batches: Vec<Batch> = Vec::new();
    for slice in wanted {
        let Some(at) = listed.slices.get(slice) else {
            continue;
        };
        match batches.iter_mut().find(|batch| batch.entry == at.entry) {
            Some(batch) => batch.slices.push((slice, at.frame)),
            None => batches.push(Batch {
                entry: at.entry.clone(),
                slices: vec![(slice, at.frame)],
            }),
        }
    }
    batches
}

impl Shared {
    /// Read one file of the source that is open; `None` when none is. A source that gives
    /// readers of their own is held only while one is made, and read through that, so that
    /// workers read at the same time; any other is held while it reads.
    pub(super) fn read_entry(&self, entry: &str) -> Option<Result<Vec<u8>, VoxaError>> {
        let mut source = lock(&self.source);
        let own = source.as_ref()?.another_reader();
        match own {
            Some(mut reader) => {
                drop(source);
                Some(reader.read(entry))
            }
            None => source.as_mut().map(|source| source.read(entry)),
        }
    }

    /// The file a batch's slices are in, opened for decoding, or why it could not be. `None`
    /// when no export is open. A file of many frames is kept for the next request.
    pub(super) fn file_of(
        &self,
        batch: &Batch,
        request: &Request,
    ) -> Option<Result<Arc<ImageFile>, String>> {
        lock(&self.source).as_ref()?;
        let open = || {
            let bytes = self
                .read_entry(&batch.entry)
                .unwrap_or_else(|| Err(VoxaError::MissingEntry(batch.entry.clone())));
            ImageFile::open(&bytes?)
        };
        let many_frames = batch.slices.iter().any(|(_, frame)| frame.is_some());
        let file = if many_frames {
            self.kept.get(&batch.entry, request.opened, open)
        } else {
            open().map(Arc::new)
        };
        Some(file.map_err(|error| error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::sync::Barrier;

    use voxa_core::SeriesId;
    use voxa_dicom::testkit::{
        EnhancedFrame, EnhancedOptions, SliceOptions, classic_slice, enhanced_series,
    };
    use voxa_dicom::{Entry, SeriesSource};

    use super::super::tests::{Bench, ONE, folder_of};
    use super::*;
    use crate::action::Action;

    /// A source that counts how often each of its entries is read.
    struct Counting {
        inner: Box<dyn SeriesSource>,
        reads: Arc<Mutex<HashMap<String, usize>>>,
    }

    impl SeriesSource for Counting {
        fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VoxaError>> + '_> {
            self.inner.entries()
        }

        fn read(&mut self, name: &str) -> Result<Vec<u8>, VoxaError> {
            let mut reads = self.reads.lock().expect("no reader panicked");
            *reads.entry(name.to_owned()).or_default() += 1;
            self.inner.read(name)
        }
    }

    /// From now on, count the reads of the source the bench has open.
    fn counted(bench: &Bench) -> Arc<Mutex<HashMap<String, usize>>> {
        let reads = Arc::new(Mutex::new(HashMap::new()));
        let mut source = lock(&bench.workers.shared.source);
        let inner = source.take().expect("a source is open");
        let counting = Counting {
            inner,
            reads: Arc::clone(&reads),
        };
        *source = Some(Box::new(counting));
        reads
    }

    /// A made-up series of `count` frames in one file.
    fn many_frames(count: i32) -> Vec<u8> {
        let frames = (0..count)
            .map(|i| EnhancedFrame::at([0.0, 0.0, f64::from(i) * 2.0]))
            .collect();
        enhanced_series(&EnhancedOptions {
            common: SliceOptions::default(),
            frames,
        })
    }

    #[test]
    fn a_file_of_many_frames_is_read_once_however_often_its_frames_are_asked_for() {
        let folder = folder_of(&[("whole.dcm".to_owned(), many_frames(12))]);
        // Room for two frames: each request has frames to decode that the last did not leave.
        let bench = Bench::with_budget(2 * ONE);
        let index = bench.index(folder.path()).expect("indexes");
        let reads = counted(&bench);
        let series = index.series[0].info.id.0.clone();
        for around in [0, 6, 11] {
            let reports = bench.frames(&index, &series, around, 2);
            let all_ready = reports
                .iter()
                .all(|report| matches!(report, Action::FrameReady { .. }));
            assert!(all_ready, "{reports:?}");
        }
        let reads = reads.lock().expect("no reader panicked");
        assert_eq!(reads.get("whole.dcm"), Some(&1), "{reads:?}");
    }

    #[test]
    fn opening_another_export_lets_go_of_the_file_that_was_held() {
        let folder = folder_of(&[("whole.dcm".to_owned(), many_frames(4))]);
        let bench = Bench::with_budget(64 * ONE);
        let index = bench.index(folder.path()).expect("indexes");
        let series = index.series[0].info.id.0.clone();
        bench.frames(&index, &series, 0, 4);
        assert!(lock(&bench.workers.shared.kept.held).is_some());
        bench.index(folder.path()).expect("indexes again");
        assert!(lock(&bench.workers.shared.kept.held).is_none());
    }

    #[test]
    fn only_the_file_asked_for_last_is_held_and_a_failure_is_tried_again() {
        let kept = Opened::default();
        let opens = Cell::new(0);
        let bytes = many_frames(2);
        let open = || {
            opens.set(opens.get() + 1);
            ImageFile::open(&bytes)
        };
        let first = kept.get("a", 1, open).expect("opens");
        let again = kept.get("a", 1, open).expect("is held");
        assert!(Arc::ptr_eq(&first, &again));
        assert_eq!(opens.get(), 1, "the second asking opened nothing");
        kept.get("b", 1, open).expect("opens");
        kept.get("a", 1, open).expect("opens again");
        assert_eq!(opens.get(), 3, "one file is held, not two");
        kept.get("a", 2, open).expect("opens");
        assert_eq!(
            opens.get(),
            4,
            "another export's file of the same name is another file"
        );
        let failing = || {
            opens.set(opens.get() + 1);
            Err(VoxaError::NotDicom)
        };
        assert!(kept.get("c", 2, failing).is_err());
        assert!(kept.get("c", 2, failing).is_err());
        assert_eq!(opens.get(), 6, "a failure is not held");
        kept.get("a", 2, open).expect("opens");
        kept.clear();
        kept.get("a", 2, open).expect("opens");
        assert_eq!(opens.get(), 8, "let go of, it is opened again");
    }

    /// A source whose every read, by it or by a reader of its own, waits at `gate` twice: once
    /// on coming in, and once more before it gives its bytes.
    struct Gated {
        gate: Arc<Barrier>,
        bytes: Vec<u8>,
    }

    impl SeriesSource for Gated {
        fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VoxaError>> + '_> {
            Box::new(std::iter::empty())
        }

        fn read(&mut self, _: &str) -> Result<Vec<u8>, VoxaError> {
            self.gate.wait();
            self.gate.wait();
            Ok(self.bytes.clone())
        }

        fn another_reader(&self) -> Option<Box<dyn SeriesSource>> {
            Some(Box::new(Self {
                gate: Arc::clone(&self.gate),
                bytes: self.bytes.clone(),
            }))
        }
    }

    #[test]
    fn a_source_that_gives_readers_of_their_own_is_not_held_while_one_reads() {
        let bench = Bench::with_budget(64 * ONE);
        let gate = Arc::new(Barrier::new(2));
        let gated = Gated {
            gate: Arc::clone(&gate),
            bytes: classic_slice(&SliceOptions::default()),
        };
        *lock(&bench.workers.shared.source) = Some(Box::new(gated));
        let shared = Arc::clone(&bench.workers.shared);
        let reading = std::thread::spawn(move || {
            let batch = Batch {
                entry: "slice.dcm".to_owned(),
                slices: vec![(0, None)],
            };
            let request = Request {
                series: SeriesId("one".to_owned()),
                around: 0,
                opened: 0,
                number: 0,
            };
            shared.file_of(&batch, &request).map(|file| file.is_ok())
        });
        gate.wait(); // the read has begun
        let free = bench.workers.shared.source.try_lock().is_ok();
        gate.wait(); // and may end
        assert_eq!(
            reading.join().expect("the reader did not panic"),
            Some(true)
        );
        assert!(free, "the source was held while a reader of its own read");
    }
}
