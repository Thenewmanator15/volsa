use std::collections::BTreeSet;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use tempfile::TempDir;
use voxa_dicom::testkit::{
    EnhancedFrame, EnhancedOptions, SliceOptions, classic_slice, enhanced_series,
};
use voxa_volume::PRESETS;

use super::*;
use crate::testing::{THICK_SCAN, id, series_files, slice_options};
use voxa_dicom::Frame;

/// Bytes held by one decoded 4 × 4 frame.
pub(super) const ONE: usize = 64;
/// Long enough for any worker on any machine; a test that waits this long has failed.
const PATIENCE: Duration = Duration::from_secs(60);
/// Long enough for a worker that was going to say something to have said it.
const QUIET: Duration = Duration::from_millis(400);

pub(super) struct Bench {
    pub(super) workers: Workers,
    reports: Receiver<Action>,
    pub(super) cache: Arc<Mutex<FrameCache>>,
    pub(super) volumes: Arc<Mutex<VolumeStore>>,
}

impl Bench {
    pub(super) fn with_budget(budget: usize) -> Self {
        Self::keeping_in(budget, None)
    }

    /// Workers with a memory budget of `budget` that keep what they keep in `local`.
    pub(super) fn keeping_in(budget: usize, local: Option<&Path>) -> Self {
        let (sender, reports) = channel();
        let cache = Arc::new(Mutex::new(FrameCache::new(budget)));
        let volumes = Arc::new(Mutex::new(VolumeStore::default()));
        let (kept, local) = (Arc::clone(&cache), local.map(Path::to_owned));
        let workers = Workers::new(sender, kept, Arc::clone(&volumes), local, None);
        Self {
            workers,
            reports,
            cache,
            volumes,
        }
    }

    pub(super) fn next(&self) -> Action {
        self.reports
            .recv_timeout(PATIENCE)
            .expect("a worker reports")
    }

    pub(super) fn is_quiet(&self) -> bool {
        self.reports.recv_timeout(QUIET).is_err()
    }

    /// Everything reported from now until the workers have been quiet for a while.
    pub(super) fn drain(&self) -> Vec<Action> {
        std::iter::from_fn(|| self.reports.recv_timeout(QUIET).ok()).collect()
    }

    /// Take reports until one is the one `wanted`, however long the workers take over it.
    pub(super) fn wait_for(&self, wanted: &Action) {
        while self.next() != *wanted {}
    }

    /// Wait for something the workers do without reporting it.
    pub(super) fn until(&self, done: impl Fn(&Self) -> bool) {
        let started = std::time::Instant::now();
        while !done(self) {
            assert!(started.elapsed() < PATIENCE, "the workers never got there");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Index a path to the end; the finished index, or the message it failed with.
    pub(super) fn index(&self, path: &Path) -> Result<Index, String> {
        self.workers.run(Effect::StartIndex(path.to_owned()), None);
        let mut latest = None;
        loop {
            match self.next() {
                Action::IndexProgress(index) => latest = Some(index),
                Action::IndexDone => return Ok(latest.expect("progress comes before done")),
                Action::IndexFailed(why) => return Err(why),
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    /// Ask for a series' frames and collect `expected` reports.
    pub(super) fn frames(
        &self,
        index: &Index,
        series: &str,
        around: usize,
        expected: usize,
    ) -> Vec<Action> {
        let wanted = Effect::RequestFrames {
            series: id(series),
            around,
        };
        self.workers.run(wanted, Some(index));
        (0..expected).map(|_| self.next()).collect()
    }

    pub(super) fn cached(&self, series: &str, slice: usize) -> Option<Arc<Frame>> {
        self.cache
            .lock()
            .expect("no worker panicked")
            .get(&id(series), slice)
    }
}

pub(super) fn folder_of(files: &[(String, Vec<u8>)]) -> TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder");
    for (name, bytes) in files {
        let path = folder.path().join(name);
        std::fs::create_dir_all(path.parent().expect("inside the folder")).expect("makes folders");
        std::fs::write(path, bytes).expect("writes");
    }
    folder
}

fn ready_slices(reports: &[Action]) -> BTreeSet<usize> {
    reports
        .iter()
        .filter_map(|report| match report {
            Action::FrameReady { slice, .. } => Some(*slice),
            _ => None,
        })
        .collect()
}

#[test]
fn indexing_a_folder_reports_what_it_found_and_then_that_it_is_done() {
    let folder = folder_of(&series_files(THICK_SCAN, 1, 30, 4));
    let bench = Bench::with_budget(100 * ONE);
    let index = bench.index(folder.path()).expect("indexes");
    assert_eq!(index.series.len(), 1);
    assert_eq!(index.series[0].slices.len(), 30);
    assert!(bench.is_quiet(), "nothing follows the end of indexing");
}

#[test]
fn every_slice_is_decoded_and_the_one_asked_for_comes_first() {
    let folder = folder_of(&series_files(THICK_SCAN, 1, 30, 4));
    let bench = Bench::with_budget(100 * ONE);
    let index = bench.index(folder.path()).expect("indexes");
    let reports = bench.frames(&index, THICK_SCAN, 12, 30);
    assert!(
        matches!(&reports[0], Action::FrameReady { series, slice: 12, .. } if *series == id(THICK_SCAN)),
        "{:?}",
        reports[0]
    );
    assert_eq!(ready_slices(&reports), (0..30).collect());
    let frame = bench
        .cached(THICK_SCAN, 12)
        .expect("in the cache before it is announced");
    assert_eq!((frame.rows, frame.cols), (4, 4));
    let Action::FrameReady { windows, .. } = &reports[0] else {
        unreachable!()
    };
    assert_eq!(*windows, PRESETS.map(|preset| preset.window(&frame.values)));
    assert!(bench.is_quiet());
}

#[test]
fn a_damaged_slice_fails_by_itself_and_the_others_still_arrive() {
    let mut files = series_files(THICK_SCAN, 1, 10, 4);
    let cut_short = SliceOptions {
        drop_pixels: 5,
        ..slice_options(THICK_SCAN, 1, 6, 4)
    };
    files[6].1 = classic_slice(&cut_short);
    let folder = folder_of(&files);
    let bench = Bench::with_budget(100 * ONE);
    let index = bench.index(folder.path()).expect("indexes");
    let reports = bench.frames(&index, THICK_SCAN, 2, 10);
    let failures: Vec<(usize, &str)> = reports
        .iter()
        .filter_map(|report| match report {
            Action::FrameFailed { slice, reason, .. } => Some((*slice, reason.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].0, 6);
    assert!(
        failures[0].1.starts_with("The image could not be decoded"),
        "{}",
        failures[0].1
    );
    assert_eq!(ready_slices(&reports).len(), 9);
    assert!(bench.cached(THICK_SCAN, 6).is_none());
}

#[test]
fn only_as_many_slices_as_the_cache_can_hold_are_decoded_nearest_first() {
    let folder = folder_of(&series_files(THICK_SCAN, 1, 30, 4));
    let bench = Bench::with_budget(5 * ONE);
    let index = bench.index(folder.path()).expect("indexes");
    let reports = bench.frames(&index, THICK_SCAN, 10, 5);
    assert_eq!(ready_slices(&reports), (8..=12).collect());
    assert!(
        bench.is_quiet(),
        "the other twenty-five would only push these out"
    );
}

#[test]
fn slices_already_in_the_cache_are_not_decoded_again() {
    let folder = folder_of(&series_files(THICK_SCAN, 1, 6, 4));
    let bench = Bench::with_budget(100 * ONE);
    let index = bench.index(folder.path()).expect("indexes");
    bench.frames(&index, THICK_SCAN, 0, 6);
    bench.workers.run(
        Effect::RequestFrames {
            series: id(THICK_SCAN),
            around: 3,
        },
        Some(&index),
    );
    assert!(bench.is_quiet());
}

#[test]
fn a_series_held_in_one_file_gives_every_frame_the_one_asked_for_first() {
    let frames = (0..6)
        .map(|i| EnhancedFrame::at([0.0, 0.0, f64::from(i) * 2.0]))
        .collect();
    let file = enhanced_series(&EnhancedOptions {
        common: SliceOptions::default(),
        frames,
    });
    let folder = folder_of(&[("enhanced.dcm".to_owned(), file)]);
    let bench = Bench::with_budget(100 * ONE);
    let index = bench.index(folder.path()).expect("indexes");
    let series = index.series[0].info.id.0.clone();
    let reports = bench.frames(&index, &series, 4, 6);
    assert!(
        matches!(&reports[0], Action::FrameReady { slice: 4, .. }),
        "{:?}",
        reports[0]
    );
    assert_eq!(ready_slices(&reports), (0..6).collect());
    // Each frame of the synthetic file is filled with its own number.
    assert_eq!(
        bench.cached(&series, 4).expect("cached").values,
        vec![4.0; 16]
    );
}

#[test]
fn a_file_that_is_neither_a_zip_nor_dicom_says_so() {
    let folder = folder_of(&[("notes.txt".to_owned(), b"not a scan".to_vec())]);
    let bench = Bench::with_budget(ONE);
    let failed = bench
        .index(&folder.path().join("notes.txt"))
        .expect_err("holds no images");
    assert_eq!(failed, "That file is not a zip or DICOM export");
}

#[test]
fn a_folder_with_no_images_says_where_it_looked() {
    let folder = folder_of(&[(
        "empty-export/readme.txt".to_owned(),
        b"nothing here".to_vec(),
    )]);
    let bench = Bench::with_budget(ONE);
    let failed = bench
        .index(&folder.path().join("empty-export"))
        .expect_err("holds no images");
    assert_eq!(failed, "No images were found in empty-export");
}

#[test]
fn a_path_that_is_not_there_fails_in_plain_words() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let bench = Bench::with_budget(ONE);
    let failed = bench
        .index(&folder.path().join("gone.zip"))
        .expect_err("there is no such file");
    assert!(failed.starts_with("The file could not be read"), "{failed}");
}

#[test]
fn thumbnails_decode_the_middle_slice_of_every_series_and_no_other() {
    let folder = folder_of(&crate::testing::three_series_files());
    let bench = Bench::with_budget(1000 * ONE);
    let index = bench.index(folder.path()).expect("indexes");
    bench.workers.run(Effect::RequestThumbnails, Some(&index));
    let reports: Vec<Action> = (0..3).map(|_| bench.next()).collect();
    let mut middles: Vec<(String, usize)> = reports
        .iter()
        .filter_map(|report| match report {
            Action::FrameReady { series, slice, .. } => Some((series.0.clone(), *slice)),
            _ => None,
        })
        .collect();
    middles.sort();
    let expected = [("1.2.3.1", 15), ("1.2.3.2", 32), ("1.2.3.3", 1)];
    assert_eq!(
        middles,
        expected.map(|(series, slice)| (series.to_owned(), slice))
    );
    assert!(bench.is_quiet(), "one slice for each series");
}

#[test]
fn the_elements_of_a_slices_file_are_listed_on_request() {
    let folder = folder_of(&series_files(THICK_SCAN, 1, 3, 4));
    let bench = Bench::with_budget(100 * ONE);
    let index = bench.index(folder.path()).expect("indexes");
    let (send, listed) = channel();
    let reply = move |rows| send.send(rows).expect("the test is waiting");
    bench.workers.list_tags(&index.series[0], 1, reply.clone());
    let rows = listed
        .recv_timeout(PATIENCE)
        .expect("answers")
        .expect("lists");
    let modality = rows
        .iter()
        .find(|row| row.keyword == "Modality")
        .expect("listed");
    assert_eq!(modality.value, "MR");
    let number = rows
        .iter()
        .find(|row| row.keyword == "InstanceNumber")
        .expect("listed");
    assert_eq!(number.value, "2", "the file of the second slice");

    bench.workers.list_tags(&index.series[0], 99, reply);
    let missing = listed.recv_timeout(PATIENCE).expect("answers");
    assert_eq!(missing, Err("There is no such slice.".to_owned()));
    assert!(
        bench.is_quiet(),
        "listing tags is not announced as an action"
    );
}

#[test]
fn a_task_that_panics_takes_neither_the_workers_nor_the_program_down() {
    let folder = folder_of(&series_files(THICK_SCAN, 1, 3, 4));
    let bench = Bench::with_budget(100 * ONE);
    bench.workers.spawn(|| panic!("a reader gave up on a file"));
    let index = bench.index(folder.path()).expect("the workers still work");
    assert_eq!(index.series.len(), 1);
}

#[test]
fn asking_for_nothing_or_for_a_series_that_is_not_there_does_nothing() {
    let folder = folder_of(&series_files(THICK_SCAN, 1, 3, 4));
    let bench = Bench::with_budget(100 * ONE);
    let index = bench.index(folder.path()).expect("indexes");
    bench.workers.run(Effect::None, Some(&index));
    bench.workers.run(
        Effect::RequestFrames {
            series: id("9.9.9"),
            around: 0,
        },
        Some(&index),
    );
    bench.workers.run(
        Effect::RequestFrames {
            series: id(THICK_SCAN),
            around: 0,
        },
        None,
    );
    assert!(bench.is_quiet());
}

#[test]
fn opening_another_export_drops_the_frames_of_the_one_before() {
    let first = folder_of(&series_files(THICK_SCAN, 1, 4, 4));
    let second = folder_of(&series_files("1.2.3.77", 7, 4, 4));
    let bench = Bench::with_budget(100 * ONE);
    let index = bench.index(first.path()).expect("indexes");
    bench.frames(&index, THICK_SCAN, 0, 4);
    assert!(bench.cached(THICK_SCAN, 0).is_some());
    let index = bench.index(second.path()).expect("indexes");
    assert!(
        bench.cached(THICK_SCAN, 0).is_none(),
        "the cache was emptied"
    );
    let reports = bench.frames(&index, "1.2.3.77", 0, 4);
    assert_eq!(
        ready_slices(&reports).len(),
        4,
        "the new export decodes from its own files"
    );
}
