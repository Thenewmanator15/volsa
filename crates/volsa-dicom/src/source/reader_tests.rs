use std::fs;
use std::io::Write;
use std::path::Path;

use super::*;
use crate::testkit::MemorySource;

/// Files of a few sizes, some larger than a zip's read-ahead and some smaller.
fn files() -> Vec<(String, Vec<u8>)> {
    (0..24usize)
        .map(|at| {
            let len = 100 + (at % 6) * 7_000;
            let bytes = (0..len)
                .map(|i| u8::try_from((i * 31 + at) % 251).expect("below 251"))
                .collect();
            (format!("IMAGES/I{at:04}"), bytes)
        })
        .collect()
}

/// A folder, a stored zip and a deflated zip of `files`.
fn sources_of(dir: &Path, files: &[(String, Vec<u8>)]) -> Vec<Box<dyn SeriesSource>> {
    let folder = dir.join("study");
    for (name, bytes) in files {
        let path = folder.join(name);
        fs::create_dir_all(path.parent().expect("has a parent")).expect("folder is made");
        fs::write(path, bytes).expect("file is written");
    }
    let mut sources = vec![open_source(&folder).expect("folder opens")];
    for method in [
        ::zip::CompressionMethod::Stored,
        ::zip::CompressionMethod::Deflated,
    ] {
        let path = dir.join(format!("{method:?}.zip"));
        let mut writer = ::zip::ZipWriter::new(fs::File::create(&path).expect("created"));
        let options = ::zip::write::SimpleFileOptions::default().compression_method(method);
        for (name, bytes) in files {
            writer
                .start_file(name.as_str(), options)
                .expect("entry starts");
            writer.write_all(bytes).expect("written");
        }
        writer.finish().expect("finished");
        sources.push(open_source(&path).expect("zip opens"));
    }
    sources
}

#[test]
fn another_reader_of_a_folder_or_a_zip_reads_what_the_source_reads() {
    let dir = tempfile::tempdir().expect("temp dir");
    let files = files();
    for mut source in sources_of(dir.path(), &files) {
        let mut reader = source.another_reader().expect("a reader of its own");
        for (name, bytes) in &files {
            assert_eq!(&reader.read(name).expect("reads"), bytes);
            assert_eq!(&source.read(name).expect("reads"), bytes);
        }
        assert!(matches!(
            reader.read("IMAGES/I9999"),
            Err(VolsaError::MissingEntry(_))
        ));
    }
}

#[test]
fn readers_of_one_source_read_on_several_threads_at_once() {
    let dir = tempfile::tempdir().expect("temp dir");
    let files = files();
    for source in sources_of(dir.path(), &files) {
        std::thread::scope(|scope| {
            for thread in 0..8 {
                let mut reader = source.another_reader().expect("a reader of its own");
                let files = &files;
                scope.spawn(move || {
                    for round in 0..3 * files.len() {
                        let (name, bytes) = &files[(round * 5 + thread) % files.len()];
                        assert_eq!(&reader.read(name).expect("reads"), bytes);
                    }
                });
            }
        });
    }
}

#[test]
fn a_source_with_no_reader_of_its_own_says_so() {
    let source = MemorySource::new(files());
    assert!(source.another_reader().is_none());
}
