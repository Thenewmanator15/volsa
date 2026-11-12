//! Reading the headers of a source's files on several threads, each through a reader of its
//! own, so that indexing a large export is not one file at a time.

use std::num::NonZero;

use volsa_core::VolsaError;

use super::{Found, REPORT_EVERY, is_directory_file};
use crate::document::{self, IndexedDocument};
use crate::header::{Header, Start, read_header_from};
use crate::nesting::{READING_STACK_BYTES, Refused};
use crate::source::{MAX_ENTRY_BYTES, SeriesSource};

/// The most threads that read headers at once.
const MOST_THREADS: usize = 8;
/// The fewest files a thread is given: fewer are read in turn.
const FEWEST_EACH: usize = 32;

/// What reading the start of one file found.
pub(super) enum Read {
    /// A `DICOMDIR`, passed over without being read.
    Directory,
    /// Not a DICOM image, nor a document.
    NotAnImage,
    /// A DICOM file with no image but a document: a report, a PDF or the like.
    Document(Box<IndexedDocument>),
    /// An image's header.
    Header(Box<Header>),
    /// A file that is not read, for the reason given: one nested too deeply to read safely.
    Refused(&'static str),
}

/// What one thread read: each file of its run, in name order, as far as the first that
/// could not be read, and why that one could not.
pub(super) type Run = (Vec<(String, Read)>, Option<VolsaError>);

/// What reading one file's start finds.
pub(super) fn read_one(name: &str, contents: &mut dyn std::io::Read) -> Result<Read, VolsaError> {
    if is_directory_file(name) {
        return Ok(Read::Directory);
    }
    let (start, bytes) = read_header_from(contents, MAX_ENTRY_BYTES)?;
    Ok(match start {
        Start::Image(header) => Read::Header(header),
        Start::NotAnImage => match document::of(name, &bytes) {
            Some(document) => Read::Document(Box::new(document)),
            None => Read::NotAnImage,
        },
        Start::Refused(why) => Read::Refused(why),
    })
}

/// Every file of `source` read on several threads, the names shared out in runs that follow
/// one another in name order. `None` when the source cannot say its names, cannot give
/// readers of its own, or has too few files to share out.
pub(super) fn read_on_threads(source: &mut dyn SeriesSource) -> Option<Vec<Run>> {
    let names = source.names()?;
    let cores = std::thread::available_parallelism().map_or(1, NonZero::get);
    let threads = cores.min(MOST_THREADS).min(names.len() / FEWEST_EACH);
    if threads < 2 {
        return None;
    }
    let readers: Option<Vec<Box<dyn SeriesSource>>> =
        (0..threads).map(|_| source.another_reader()).collect();
    let readers = readers?;
    let each = names.len().div_ceil(threads);
    let runs = std::thread::scope(|scope| {
        // These threads parse headers, so each has the stack every reading thread has.
        let started: Vec<_> = names
            .chunks(each)
            .zip(readers)
            .map(|(run, mut reader)| {
                std::thread::Builder::new()
                    .stack_size(READING_STACK_BYTES)
                    .spawn_scoped(scope, move || read_run(reader.as_mut(), run))
            })
            .collect();
        started
            .into_iter()
            .map(|thread| match thread {
                Ok(thread) => thread.join().unwrap_or_else(|_| stopped()),
                Err(_) => stopped(),
            })
            .collect()
    });
    Some(runs)
}

/// The files of one run, read through `reader`, as far as the first that cannot be read.
fn read_run(reader: &mut dyn SeriesSource, names: &[String]) -> Run {
    let mut read = Vec::with_capacity(names.len());
    let ended = reader.visit_these(names, &mut |name, contents| {
        read.push((name.to_owned(), read_one(name, contents)?));
        Ok(())
    });
    (read, ended.err())
}

/// What a run that stopped part-way is taken to have read: nothing, and that it could not.
fn stopped() -> Run {
    let why = std::io::Error::other("a thread reading the export stopped");
    (Vec::new(), Some(VolsaError::Io(why)))
}

impl Found {
    /// Take in what reading one file found, the file visited `at`th. Whether it is time to
    /// say how far indexing has got.
    pub(super) fn take(&mut self, name: String, read: Read, at: usize) -> bool {
        match read {
            Read::Directory => false,
            Read::NotAnImage => {
                self.skipped.push(name);
                false
            }
            Read::Document(document) => {
                self.documents.push(*document);
                false
            }
            Read::Refused(why) => {
                self.refused.push(Refused {
                    entry: name,
                    reason: why.to_owned(),
                });
                false
            }
            Read::Header(header) => self.add(name, *header) || at.is_multiple_of(REPORT_EVERY),
        }
    }
}
