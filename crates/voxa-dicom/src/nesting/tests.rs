//! A file nested too deeply is refused before the DICOM library builds it, on every path that
//! hands the library bytes, and the rest of an export still opens.
//!
//! Every read here is made on a thread with a small stack, so that what keeps it from
//! overflowing is the check on nesting, not the room Voxa's reading threads are given.

use std::fmt::Debug;
use std::io::{Cursor, Write};
use std::path::Path;

use dicom_core::value::DataSetSequence;
use dicom_core::{DataElement, PrimitiveValue, VR};
use dicom_dictionary_std::tags;
use dicom_object::InMemDicomObject;
use voxa_core::VoxaError;

use super::MAX_NESTING;
use super::Refused;
use crate::SliceRef;
use crate::decode::ImageFile;
use crate::header::{Start, read_header_from};
use crate::index::{Index, index};
use crate::source::{MAX_ENTRY_BYTES, SeriesSource, open_source};
use crate::tags::list_tags;
use crate::testkit::{MemorySource, Nesting, SliceOptions, classic_slice, nested_slice};

/// The stack of a Windows program's main thread: half what a spawned thread has by default.
const SMALL_STACK: usize = 1 << 20;

/// Deeper than any thread's stack could hold if the library were left to build it: about
/// 150 MB of stack in a debug build, and 25 MB in a release build.
const FAR_TOO_DEEP: usize = 20_000;

/// What a refused file is said to be.
const WHY: &str = "nested too deeply to read safely";

/// Do `work` on a thread with a small stack, and give what it gives.
fn on_a_small_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(SMALL_STACK)
        .spawn(work)
        .expect("a thread is made")
        .join()
        .expect("the reading did not panic")
}

/// The four ways nesting can be written: implicit or explicit VR, each sequence and item with
/// its length given or closed by a delimiter.
fn every_encoding(depth: usize) -> Vec<Nesting> {
    [(false, false), (false, true), (true, false), (true, true)]
        .into_iter()
        .map(|(implicit, defined_lengths)| Nesting {
            implicit,
            defined_lengths,
            ..Nesting::of(depth)
        })
        .collect()
}

fn named(nesting: &Nesting) -> String {
    format!(
        "{} sequences deep, {} VR, {}",
        nesting.depth,
        if nesting.implicit {
            "implicit"
        } else {
            "explicit"
        },
        if nesting.defined_lengths {
            "lengths given"
        } else {
            "delimited"
        }
    )
}

/// What a header read, as indexing makes it, finds at the start of `bytes`.
fn header_of(bytes: &[u8]) -> Start {
    read_header_from(&mut &bytes[..], MAX_ENTRY_BYTES).expect("the bytes are read")
}

/// The file opened whole and its image decoded, as a viewport shows it.
fn opened(bytes: &[u8]) -> Result<(), VoxaError> {
    ImageFile::open(bytes)?.decode(None).map(|_| ())
}

/// The elements of the file listed for the tag browser: how many there are.
fn tags_of(bytes: Vec<u8>) -> Result<usize, VoxaError> {
    let mut source = MemorySource::new(vec![("one.dcm".to_owned(), bytes)]);
    let slice = SliceRef {
        entry: "one.dcm".to_owned(),
        frame: None,
    };
    list_tags(&mut source, &slice).map(|rows| rows.len())
}

fn is_too_deep<T: Debug>(result: &Result<T, VoxaError>) -> bool {
    matches!(result, Err(VoxaError::Decode(why)) if why.contains(WHY))
}

#[test]
fn deep_nesting_written_any_way_is_refused_by_a_header_read() {
    on_a_small_stack(|| {
        for nesting in every_encoding(FAR_TOO_DEEP) {
            let found = header_of(&nested_slice(&nesting));
            assert_eq!(found, Start::Refused(WHY), "{}", named(&nesting));
        }
    });
}

#[test]
fn deep_nesting_written_any_way_is_refused_when_a_file_is_read_whole() {
    on_a_small_stack(|| {
        for nesting in every_encoding(FAR_TOO_DEEP) {
            let bytes = nested_slice(&nesting);
            let decoded = opened(&bytes);
            assert!(is_too_deep(&decoded), "{}: {decoded:?}", named(&nesting));
            let listed = tags_of(bytes);
            assert!(is_too_deep(&listed), "{}: {listed:?}", named(&nesting));
        }
    });
}

#[test]
fn nesting_one_past_the_limit_is_refused() {
    on_a_small_stack(|| {
        for nesting in every_encoding(MAX_NESTING + 1) {
            let bytes = nested_slice(&nesting);
            assert_eq!(
                header_of(&bytes),
                Start::Refused(WHY),
                "{}",
                named(&nesting)
            );
            let decoded = opened(&bytes);
            assert!(is_too_deep(&decoded), "{}: {decoded:?}", named(&nesting));
        }
    });
}

#[test]
fn nesting_as_deep_as_the_limit_reads_like_any_other_file() {
    on_a_small_stack(|| {
        for nesting in every_encoding(MAX_NESTING) {
            let bytes = nested_slice(&nesting);
            let Start::Image(header) = header_of(&bytes) else {
                panic!("{} was not read as an image", named(&nesting));
            };
            assert_eq!((header.rows, header.cols), (4, 4), "{}", named(&nesting));
            opened(&bytes).unwrap_or_else(|why| panic!("{}: {why}", named(&nesting)));
            tags_of(bytes).unwrap_or_else(|why| panic!("{}: {why}", named(&nesting)));
        }
    });
}

#[test]
fn nesting_after_the_pixels_is_refused_when_the_whole_file_is_read() {
    on_a_small_stack(|| {
        for nesting in every_encoding(FAR_TOO_DEEP) {
            let after = Nesting {
                after_pixels: true,
                ..nesting
            };
            let bytes = nested_slice(&after);
            // A header read stops at the pixels, so it never meets what lies after them.
            let found = header_of(&bytes);
            assert!(
                matches!(found, Start::Image(_)),
                "{}: {found:?}",
                named(&after)
            );
            let decoded = opened(&bytes);
            assert!(is_too_deep(&decoded), "{}: {decoded:?}", named(&after));
            let listed = tags_of(bytes);
            assert!(is_too_deep(&listed), "{}: {listed:?}", named(&after));
        }
    });
}

#[test]
fn nesting_that_begins_past_the_first_read_is_still_refused() {
    on_a_small_stack(|| {
        for nesting in every_encoding(FAR_TOO_DEEP) {
            let padded = Nesting {
                padding: 150_000,
                ..nesting
            };
            let bytes = nested_slice(&padded);
            assert_eq!(header_of(&bytes), Start::Refused(WHY), "{}", named(&padded));
            let decoded = opened(&bytes);
            assert!(is_too_deep(&decoded), "{}: {decoded:?}", named(&padded));
        }
    });
}

/// The same file with its data set deflated, as the deflated transfer syntax stores it.
fn deflated(bytes: &[u8]) -> Vec<u8> {
    let mut meta =
        dicom_object::FileMetaTable::from_reader(&bytes[128..]).expect("the header is read");
    let data_set = 128 + 4 + 12 + meta.information_group_length as usize;
    meta.transfer_syntax = "1.2.840.10008.1.2.1.99".to_owned();
    meta.update_information_group_length();
    let mut out = vec![0u8; 128];
    out.extend_from_slice(b"DICM");
    meta.write(&mut out)
        .expect("writing to memory does not fail");
    let mut squeezed = flate2::write::DeflateEncoder::new(out, flate2::Compression::best());
    squeezed
        .write_all(&bytes[data_set..])
        .expect("writing to memory does not fail");
    squeezed.finish().expect("writing to memory does not fail")
}

#[test]
fn a_deflated_file_is_inflated_as_the_library_would_and_refused() {
    on_a_small_stack(|| {
        let shallow = deflated(&nested_slice(&Nesting::of(3)));
        assert!(matches!(header_of(&shallow), Start::Image(_)));
        opened(&shallow).expect("a shallow deflated file opens");
        let deep = deflated(&nested_slice(&Nesting::of(5 * FAR_TOO_DEEP)));
        assert!(deep.len() < 50_000, "{} bytes", deep.len());
        assert_eq!(header_of(&deep), Start::Refused(WHY));
        assert!(is_too_deep(&opened(&deep)));
    });
}

#[test]
fn many_sequences_side_by_side_are_not_deep() {
    on_a_small_stack(|| {
        let mut file =
            dicom_object::from_reader(Cursor::new(classic_slice(&SliceOptions::default())))
                .expect("parses");
        let purpose = |n: usize| {
            let mut code = InMemDicomObject::new_empty();
            code.put(DataElement::new(
                tags::CODE_VALUE,
                VR::SH,
                PrimitiveValue::from(format!("{n}")),
            ));
            let mut item = InMemDicomObject::new_empty();
            item.put(DataElement::new(
                tags::PURPOSE_OF_REFERENCE_CODE_SEQUENCE,
                VR::SQ,
                DataSetSequence::from(vec![code]),
            ));
            item
        };
        file.put(DataElement::new(
            tags::REFERENCED_IMAGE_SEQUENCE,
            VR::SQ,
            DataSetSequence::from((0..3_000).map(purpose).collect::<Vec<_>>()),
        ));
        let mut bytes = Vec::new();
        file.write_all(&mut bytes).expect("writes to memory");
        assert!(matches!(header_of(&bytes), Start::Image(_)));
        opened(&bytes).expect("a wide file opens");
    });
}

/// Where the nesting in `bytes` begins: the first content sequence.
fn nesting_starts(bytes: &[u8]) -> usize {
    let tag = [0x40, 0x00, 0x30, 0xA7];
    bytes
        .windows(4)
        .position(|four| four == tag)
        .expect("the file has a content sequence")
}

#[test]
fn a_deep_file_cut_short_anywhere_is_read_safely_and_refused_once_past_the_limit() {
    on_a_small_stack(|| {
        for nesting in every_encoding(300) {
            let bytes = nested_slice(&nesting);
            // The bytes that open one more sequence and its item.
            let level = if nesting.implicit { 16 } else { 20 };
            let past_the_limit = nesting_starts(&bytes) + (MAX_NESTING + 1) * level;
            for cut in (0..bytes.len()).step_by(29) {
                // Cut before the limit, what is read is whatever the library makes of it: a
                // header that ended before its nesting is still an image's.
                let found = header_of(&bytes[..cut]);
                if cut >= past_the_limit {
                    assert_eq!(
                        found,
                        Start::Refused(WHY),
                        "{} cut at {cut}",
                        named(&nesting)
                    );
                }
            }
        }
    });
}

/// `count` slices of one made-up series.
fn series(count: i32) -> Vec<(String, Vec<u8>)> {
    (0..count)
        .map(|place| {
            let slice = classic_slice(&SliceOptions {
                instance_number: place + 1,
                position: Some([-100.0, -120.0, f64::from(place) * 5.0]),
                ..SliceOptions::default()
            });
            (format!("IMAGES/I{place:04}"), slice)
        })
        .collect()
}

/// Slices in the made-up export: enough that an index which shares a large export out among
/// several threads would share out this one too.
const SLICES: i32 = 100;

/// Files of an export: the slices of a series, a deeply nested file and a note.
fn export() -> Vec<(String, Vec<u8>)> {
    let mut files = series(SLICES);
    files.push((
        "IMAGES/DEEP".to_owned(),
        nested_slice(&Nesting::of(FAR_TOO_DEEP)),
    ));
    files.push(("notes.txt".to_owned(), b"a note beside the scans".to_vec()));
    files
}

/// The index of the export says the deep file was refused and why, and holds the series.
fn assert_refused_and_the_rest_read(index: &Index) {
    assert_eq!(
        index.refused,
        vec![Refused {
            entry: "IMAGES/DEEP".to_owned(),
            reason: WHY.to_owned(),
        }]
    );
    assert_eq!(index.skipped, vec!["notes.txt".to_owned()]);
    assert_eq!(index.series.len(), 1);
    let slices = usize::try_from(SLICES).expect("a count");
    assert_eq!(index.series[0].slices.len(), slices);
}

fn indexed(source: &mut dyn SeriesSource) -> Index {
    index(source, &mut |_| {}).expect("indexing does not fail")
}

#[test]
fn an_index_lists_a_refused_file_with_why_and_reads_the_rest() {
    on_a_small_stack(|| {
        let found = indexed(&mut MemorySource::new(export()));
        assert_refused_and_the_rest_read(&found);
    });
}

fn write(root: &Path, name: &str, bytes: &[u8]) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("folder is made");
    std::fs::write(path, bytes).expect("file is written");
}

#[test]
fn a_refused_file_in_a_folder_is_listed_and_the_rest_of_the_folder_opens() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    for (name, bytes) in export() {
        write(folder.path(), &name, &bytes);
    }
    let path = folder.path().to_owned();
    on_a_small_stack(move || {
        let mut source = open_source(&path).expect("the folder opens");
        assert_refused_and_the_rest_read(&indexed(source.as_mut()));
    });
}

#[test]
fn a_refused_file_in_a_zip_is_listed_and_the_rest_of_the_zip_opens() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let path = folder.path().join("export.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).expect("zip is created"));
    for (name, bytes) in export() {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .expect("entry starts");
        zip.write_all(&bytes).expect("entry is written");
    }
    zip.finish().expect("zip is finished");
    on_a_small_stack(move || {
        let mut source = open_source(&path).expect("the zip opens");
        assert_refused_and_the_rest_read(&indexed(source.as_mut()));
    });
}
