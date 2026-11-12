//! Indexing reads only as much of each file as its header needs, and finds what reading every
//! file whole found.

use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dicom_core::{DataElement, PrimitiveValue, Tag, VR};
use dicom_dictionary_std::{tags, uids};
use dicom_object::{FileMetaTableBuilder, InMemDicomObject};
use volsa_core::VolsaError;

use super::tests::stack;
use super::*;
use crate::header::read_header;
use crate::source::{Entry, open_source};
use crate::testkit::{
    EXPLICIT_VR_LITTLE_ENDIAN, EnhancedFrame, EnhancedOptions, MemorySource, SliceOptions,
    classic_slice, enhanced_series,
};

/// What `index` gave before it read only headers: every file read whole, in name order.
fn index_of_whole_files(files: &[(String, Vec<u8>)], reports: &mut Vec<Index>) -> Index {
    let mut files = files.to_vec();
    files.sort();
    let mut found = Found::default();
    for (seen, (name, bytes)) in files.into_iter().enumerate() {
        if is_directory_file(&name) {
            continue;
        }
        let Some(header) = read_header(&bytes) else {
            found.skipped.push(name);
            continue;
        };
        if found.add(name, header) || seen % REPORT_EVERY == 0 {
            reports.push(found.index());
        }
    }
    let finished = found.index();
    reports.push(finished.clone());
    finished
}

/// A `DICOMDIR`: a list of the files beside it, with no image of its own.
fn dicomdir() -> Vec<u8> {
    let mut obj = InMemDicomObject::new_empty();
    let class = PrimitiveValue::from(uids::MEDIA_STORAGE_DIRECTORY_STORAGE);
    obj.put(DataElement::new(tags::SOP_CLASS_UID, VR::UI, class));
    let instance = PrimitiveValue::from("1.2.826.0.1.3680043.9.9999.77");
    obj.put(DataElement::new(tags::SOP_INSTANCE_UID, VR::UI, instance));
    let meta = FileMetaTableBuilder::new()
        .transfer_syntax(EXPLICIT_VR_LITTLE_ENDIAN)
        .implementation_class_uid("1.2.826.0.1.3680043.9.9999.0");
    let file = obj.with_meta(meta).expect("has its identifiers");
    let mut bytes = Vec::new();
    file.write_all(&mut bytes).expect("writes to memory");
    bytes
}

/// A slice of 512 by 512 whose header holds `extra` more bytes than an ordinary one's.
fn large_slice(place: i32, extra: usize) -> Vec<u8> {
    let options = SliceOptions {
        series_uid: "1.2.826.0.1.3680043.9.9999.1.512".to_owned(),
        series_number: 3,
        instance_number: place + 1,
        rows: 512,
        cols: 512,
        position: Some([-100.0, -120.0, f64::from(place) * 2.0]),
        ..SliceOptions::default()
    };
    let bytes = classic_slice(&options);
    if extra == 0 {
        return bytes;
    }
    let mut file = dicom_object::from_reader(Cursor::new(bytes)).expect("parses");
    file.put(DataElement::new(
        Tag(0x0029, 0x0010),
        VR::LO,
        PrimitiveValue::from("MADE UP"),
    ));
    let padding = PrimitiveValue::from(vec![0x5Au8; extra]);
    file.put(DataElement::new(Tag(0x0029, 0x1010), VR::OB, padding));
    let mut out = Vec::new();
    file.write_all(&mut out).expect("writes to memory");
    out
}

/// A made-up study: a stack of small slices, a stack of large ones (one with a header longer
/// than the first part read), a series in one file whose header runs long, a `DICOMDIR`, and
/// files that are not images.
pub(super) fn made_up_study() -> Vec<(String, Vec<u8>)> {
    let mut files = stack(40, 2.5, &SliceOptions::default());
    for place in 0..6 {
        let extra = if place == 4 { 150_000 } else { 0 };
        files.push((format!("IMAGES/S3/I{place:04}"), large_slice(place, extra)));
    }
    let frames = (0..700)
        .map(|i| EnhancedFrame::at([0.0, 0.0, f64::from(i) * 1.5]))
        .collect();
    let enhanced = EnhancedOptions {
        common: SliceOptions {
            series_uid: "1.2.826.0.1.3680043.9.9999.1.700".to_owned(),
            series_number: 9,
            rows: 32,
            cols: 32,
            ..SliceOptions::default()
        },
        frames,
    };
    files.push(("IMAGES/S9/WHOLE".to_owned(), enhanced_series(&enhanced)));
    files.push(("DICOMDIR".to_owned(), dicomdir()));
    files.push(("README.TXT".to_owned(), b"made up for a test".to_vec()));
    let noise = (0..300_000u32).map(|i| (i * 13 % 251) as u8).collect();
    files.push(("IMAGES/NOISE".to_owned(), noise));
    files
}

pub(super) fn folder_of(root: &Path, files: &[(String, Vec<u8>)]) {
    for (name, bytes) in files {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().expect("has a parent")).expect("folder made");
        std::fs::write(path, bytes).expect("written");
    }
}

pub(super) fn zip_of(path: &Path, files: &[(String, Vec<u8>)], method: zip::CompressionMethod) {
    let mut writer = zip::ZipWriter::new(std::fs::File::create(path).expect("created"));
    let options = zip::write::SimpleFileOptions::default().compression_method(method);
    for (name, bytes) in files {
        writer
            .start_file(name.as_str(), options)
            .expect("entry starts");
        writer.write_all(bytes).expect("written");
    }
    writer.finish().expect("finished");
}

#[test]
fn indexing_by_headers_finds_and_reports_what_reading_whole_files_did() {
    let files = made_up_study();
    let mut expected_reports = Vec::new();
    let expected = index_of_whole_files(&files, &mut expected_reports);
    let shapes: Vec<(usize, usize)> = expected
        .series
        .iter()
        .map(|s| (s.info.slices, s.info.problems.len()))
        .collect();
    assert_eq!(shapes, vec![(40, 0), (6, 0), (700, 0)]);
    assert_eq!(expected.skipped, vec!["IMAGES/NOISE", "README.TXT"]);
    let dir = tempfile::tempdir().expect("temp dir");
    folder_of(&dir.path().join("study"), &files);
    zip_of(
        &dir.path().join("stored.zip"),
        &files,
        zip::CompressionMethod::Stored,
    );
    zip_of(
        &dir.path().join("deflated.zip"),
        &files,
        zip::CompressionMethod::Deflated,
    );
    let mut sources = vec![Box::new(MemorySource::new(files.clone())) as Box<dyn SeriesSource>];
    for name in ["study", "stored.zip", "deflated.zip", "study/DICOMDIR"] {
        sources.push(open_source(&dir.path().join(name)).expect("opens"));
    }
    for mut source in sources {
        let mut reports = Vec::new();
        let found =
            index(source.as_mut(), &mut |so_far| reports.push(so_far.clone())).expect("indexes");
        assert_eq!(found, expected);
        assert_eq!(reports, expected_reports);
    }
}

/// A source in memory that counts the bytes it hands over, whichever way it is read.
struct Counting {
    inner: MemorySource,
    given: Arc<AtomicUsize>,
}

/// A reader that adds what it reads to a count.
struct Tally<'a> {
    inner: &'a [u8],
    given: &'a AtomicUsize,
}

impl Read for Tally<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(out)?;
        self.given.fetch_add(count, Ordering::SeqCst);
        Ok(count)
    }
}

impl SeriesSource for Counting {
    fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VolsaError>> + '_> {
        let given = Arc::clone(&self.given);
        Box::new(self.inner.entries().inspect(move |entry| {
            if let Ok(entry) = entry {
                given.fetch_add(entry.bytes.len(), Ordering::SeqCst);
            }
        }))
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, VolsaError> {
        self.inner.read(name)
    }

    fn visit_files(
        &mut self,
        visit: &mut dyn FnMut(&str, &mut dyn Read) -> Result<(), VolsaError>,
    ) -> Result<(), VolsaError> {
        let given = Arc::clone(&self.given);
        self.inner.visit_files(&mut |name, contents| {
            let mut whole = Vec::new();
            contents.read_to_end(&mut whole)?;
            let mut tally = Tally {
                inner: &whole,
                given: &given,
            };
            visit(name, &mut tally)
        })
    }
}

#[test]
fn indexing_reads_only_the_start_of_each_file() {
    let files: Vec<(String, Vec<u8>)> = (0..20)
        .map(|place| (format!("IMAGES/I{place:04}"), large_slice(place, 0)))
        .collect();
    let whole: usize = files.iter().map(|(_, bytes)| bytes.len()).sum();
    let given = Arc::new(AtomicUsize::new(0));
    let mut source = Counting {
        inner: MemorySource::new(files),
        given: Arc::clone(&given),
    };
    let found = index(&mut source, &mut |_| {}).expect("indexes");
    assert_eq!(found.series[0].info.slices, 20);
    let given = given.load(Ordering::SeqCst);
    assert!(given <= 20 * 64 * 1024, "{given} of {whole} bytes read");
}

#[test]
fn a_zip_entry_damaged_past_its_header_is_indexed_and_fails_when_it_is_read() {
    let files: Vec<(String, Vec<u8>)> = (0..3)
        .map(|place| (format!("IMAGES/I{place:04}"), large_slice(place, 0)))
        .collect();
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("export.zip");
    zip_of(&path, &files, zip::CompressionMethod::Stored);
    let mut bytes = std::fs::read(&path).expect("read back");
    let data_start = zip::ZipArchive::new(Cursor::new(bytes.clone()))
        .expect("opens")
        .by_index_raw(1)
        .expect("the second slice")
        .data_start()
        .expect("its data are found");
    let deep = usize::try_from(data_start).expect("small") + 300_000;
    bytes[deep] ^= 0xFF;
    std::fs::write(&path, bytes).expect("written");
    let mut source = open_source(&path).expect("opens");
    let found = index(source.as_mut(), &mut |_| {}).expect("only headers are read");
    assert_eq!(found.series[0].info.slices, 3);
    let damaged = source.read("IMAGES/I0001");
    assert!(
        damaged.is_err(),
        "the damage is found when the slice is read"
    );
    assert!(source.read("IMAGES/I0002").is_ok());
}
