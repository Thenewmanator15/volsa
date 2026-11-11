use std::io::{Cursor, Read, Seek, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::*;
use crate::source::open_source;

/// What the zip library says of one entry, or of why it cannot be read.
type Said = Result<(String, u64, Option<u64>, u64, u64, u32), String>;

/// Everything the zip library says of an archive and its entries: two openings that agree on
/// all of it read the same files from the same places, or fail on the same ones.
type Summary = (u64, u64, Vec<u8>, Vec<Said>);

fn summary<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Summary {
    let entries = (0..archive.len())
        .map(|at| {
            let entry = archive.by_index_raw(at).map_err(|e| e.to_string())?;
            Ok((
                entry.name().map_err(|e| e.to_string())?.into_owned(),
                entry.header_start(),
                entry.data_start(),
                entry.compressed_size(),
                entry.size(),
                entry.crc32(),
            ))
        })
        .collect();
    (
        archive.offset(),
        archive.central_directory_start(),
        archive.comment().to_vec(),
        entries,
    )
}

/// A zip of a few made-up files, some stored and some deflated, with `finish` given the writer
/// last to change the archive's end.
fn made_up_zip(finish: impl FnOnce(&mut ZipWriter<Cursor<Vec<u8>>>)) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer.add_directory("IMAGES/", stored).expect("folder");
    for at in 0..12u8 {
        let options = if at % 2 == 0 { stored } else { deflated };
        writer
            .start_file(format!("IMAGES/I{at:04}"), options)
            .expect("entry starts");
        writer.write_all(&[at; 3000]).expect("written");
    }
    finish(&mut writer);
    writer.finish().expect("finished").into_inner()
}

/// How Voxa opens `bytes` as a zip file, and whether what it found is what the zip library's
/// own search of the file finds, which is how every zip was opened before.
fn opened_as_the_search_opens(bytes: &[u8]) -> Opening {
    let mut file = tempfile::tempfile().expect("a temporary file");
    file.write_all(bytes).expect("written");
    let (mut archive, opening) = open_archive(file).expect("opens");
    let mut searched = ZipArchive::new(Cursor::new(bytes.to_vec())).expect("the search opens it");
    assert_eq!(summary(&mut archive), summary(&mut searched));
    opening
}

#[test]
fn an_ordinary_zip_is_opened_as_its_end_record_says_and_finds_what_a_search_finds() {
    let bytes = made_up_zip(|_| {});
    assert_eq!(opened_as_the_search_opens(&bytes), Opening::AsItsEndSays);
}

#[test]
fn a_zip64_archive_is_opened_as_its_end_record_says_too() {
    let bytes = made_up_zip(|writer| writer.set_raw_zip64_extensible_data_sector(Box::new([])));
    let zip64_end = b"PK\x06\x06";
    assert!(
        bytes.windows(4).any(|four| four == zip64_end),
        "a zip64 end was written"
    );
    assert_eq!(opened_as_the_search_opens(&bytes), Opening::AsItsEndSays);
}

#[test]
fn an_empty_zip_is_opened_as_its_end_record_says() {
    let bytes = ZipWriter::new(Cursor::new(Vec::new()))
        .finish()
        .expect("finished")
        .into_inner();
    assert_eq!(opened_as_the_search_opens(&bytes), Opening::AsItsEndSays);
}

/// A zip with something before it that starts as a zip entry does, so Voxa takes it for a zip.
fn with_data_before_it() -> Vec<u8> {
    let mut bytes = b"PK\x03\x04".to_vec();
    bytes.extend((0..2000u32).map(|i| (i % 7) as u8));
    bytes.extend(made_up_zip(|_| {}));
    bytes
}

#[test]
fn a_zip_with_data_before_it_is_found_by_searching() {
    assert_eq!(
        opened_as_the_search_opens(&with_data_before_it()),
        Opening::BySearching
    );
}

#[test]
fn the_files_of_a_zip_with_data_before_it_are_read_as_before() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("export.zip");
    std::fs::write(&path, with_data_before_it()).expect("written");
    let mut source = open_source(&path).expect("opens");
    let entries: Vec<Entry> = source
        .entries()
        .collect::<Result<_, _>>()
        .expect("every entry reads");
    assert_eq!(entries.len(), 12);
    for (at, entry) in (0..12u8).zip(&entries) {
        assert_eq!(entry.name, format!("IMAGES/I{at:04}"));
        assert_eq!(entry.bytes, vec![at; 3000]);
    }
}

#[test]
fn a_second_end_record_near_the_end_is_left_to_the_search() {
    // Two zips one after the other: the first one's end is near enough the end of the file
    // to be taken for the end of an archive that starts at the file's start.
    let mut bytes = made_up_zip(|_| {});
    let mut second = ZipWriter::new(Cursor::new(Vec::new()));
    second
        .start_file("other", SimpleFileOptions::default())
        .expect("entry starts");
    second.write_all(b"other").expect("written");
    bytes.extend(second.finish().expect("finished").into_inner());
    assert_eq!(opened_as_the_search_opens(&bytes), Opening::BySearching);
}

#[test]
fn an_end_record_signature_in_the_comment_is_left_to_the_search() {
    let bytes = made_up_zip(|writer| {
        writer
            .set_raw_comment(b"made up PK\x05\x06 comment".to_vec().into_boxed_slice())
            .expect("the comment fits");
    });
    assert_eq!(opened_as_the_search_opens(&bytes), Opening::BySearching);
}
