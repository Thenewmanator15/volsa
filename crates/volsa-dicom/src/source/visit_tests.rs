use std::fs;
use std::io::Write;
use std::path::Path;

use super::*;
use crate::testkit::MemorySource;

/// What visiting `source` hands over when every file is read to its end.
fn visited_whole(source: &mut dyn SeriesSource) -> Vec<(String, Vec<u8>)> {
    let mut seen = Vec::new();
    source
        .visit_files(&mut |name, contents| {
            let mut bytes = Vec::new();
            contents.read_to_end(&mut bytes)?;
            seen.push((name.to_owned(), bytes));
            Ok(())
        })
        .expect("every file is visited");
    seen
}

fn listed(source: &mut dyn SeriesSource) -> Vec<(String, Vec<u8>)> {
    source
        .entries()
        .map(|entry| entry.map(|e| (e.name, e.bytes)))
        .collect::<Result<_, _>>()
        .expect("every entry reads")
}

const FILES: [(&str, &[u8]); 4] = [
    ("DICOMDIR", b"index"),
    ("IMAGES/S1/I0002", b"two"),
    ("IMAGES/S1/I0001", b"one"),
    ("b.dcm", b"bee"),
];

fn folder_of_files(root: &Path) {
    for (name, bytes) in FILES {
        let path = root.join(name);
        fs::create_dir_all(path.parent().expect("has a parent")).expect("folder is made");
        fs::write(path, bytes).expect("file is written");
    }
}

fn zip_of_files(path: &Path, method: ::zip::CompressionMethod) {
    let mut writer = ::zip::ZipWriter::new(fs::File::create(path).expect("zip is created"));
    let options = ::zip::write::SimpleFileOptions::default().compression_method(method);
    for (name, bytes) in FILES {
        writer.start_file(name, options).expect("entry starts");
        writer.write_all(bytes).expect("entry is written");
    }
    writer.finish().expect("zip is finished");
}

/// A folder, a stored zip, a deflated zip and a source in memory, all of the same files.
fn every_kind_of_source(dir: &Path) -> Vec<Box<dyn SeriesSource>> {
    let folder = dir.join("study");
    folder_of_files(&folder);
    let stored = dir.join("stored.zip");
    zip_of_files(&stored, ::zip::CompressionMethod::Stored);
    let deflated = dir.join("deflated.zip");
    zip_of_files(&deflated, ::zip::CompressionMethod::Deflated);
    let in_memory = FILES
        .iter()
        .map(|(name, bytes)| ((*name).to_owned(), bytes.to_vec()))
        .collect();
    vec![
        open_source(&folder).expect("folder opens"),
        open_source(&stored).expect("zip opens"),
        open_source(&deflated).expect("zip opens"),
        Box::new(MemorySource::new(in_memory)),
    ]
}

#[test]
fn a_visit_hands_over_every_file_in_the_order_and_with_the_contents_entries_gives() {
    let dir = tempfile::tempdir().expect("temp dir");
    for mut source in every_kind_of_source(dir.path()) {
        let whole = listed(source.as_mut());
        assert_eq!(whole.len(), 4);
        assert_eq!(visited_whole(source.as_mut()), whole);
    }
}

#[test]
fn a_single_file_is_visited_as_its_entry_is_listed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("slice.dcm");
    fs::write(&path, b"not a zip").expect("written");
    let mut source = open_source(&path).expect("opens");
    assert_eq!(visited_whole(source.as_mut()), listed(source.as_mut()));
}

#[test]
fn a_visitor_may_read_only_the_start_of_each_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    for mut source in every_kind_of_source(dir.path()) {
        let mut starts = Vec::new();
        source
            .visit_files(&mut |name, contents| {
                let mut first = [0u8; 1];
                contents.read_exact(&mut first)?;
                starts.push((name.to_owned(), first[0]));
                Ok(())
            })
            .expect("every file is visited");
        let expected: Vec<(String, u8)> = listed(source.as_mut())
            .into_iter()
            .map(|(name, bytes)| (name, bytes[0]))
            .collect();
        assert_eq!(starts, expected);
    }
}

#[test]
fn an_error_from_the_visitor_ends_the_visit_with_that_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    for mut source in every_kind_of_source(dir.path()) {
        let mut visits = 0;
        let outcome = source.visit_files(&mut |_, _| {
            visits += 1;
            if visits == 2 {
                Err(VolsaError::NotDicom)
            } else {
                Ok(())
            }
        });
        assert!(matches!(outcome, Err(VolsaError::NotDicom)), "{outcome:?}");
        assert_eq!(visits, 2, "no file after the error is visited");
    }
}

#[test]
fn a_file_over_the_limit_is_refused_in_the_words_reading_it_whole_refused_it() {
    let reading_whole = read_limited(&mut [7u8; 100].as_slice(), 99).expect_err("too big");
    assert_eq!(too_large(99).to_string(), reading_whole.to_string());
    assert!(within_limit(99, 99).is_ok());
    assert_eq!(
        within_limit(100, 99).expect_err("too big").to_string(),
        reading_whole.to_string()
    );
}
