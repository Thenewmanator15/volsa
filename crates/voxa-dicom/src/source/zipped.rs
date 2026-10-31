//! The files inside a zip archive, read straight from the archive.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;

use voxa_core::VoxaError;
use zip::ZipArchive;
use zip::read::{ArchiveOffset, Config};
use zip::result::ZipResult;

use super::archive_file::ArchiveFile;
use super::{Entry, MAX_ENTRY_BYTES, SeriesSource, read_limited, within_limit};

/// The files inside a zip archive, read straight from the archive. Its other readers share
/// the open file and what was read of the archive's directory, each reading where it likes.
pub(super) struct ZipSource {
    archive: ZipArchive<ArchiveFile>,
    /// (name with `/`, position in the archive) of every file, sorted by name.
    files: Arc<[(String, usize)]>,
}

impl ZipSource {
    pub(super) fn open(path: &Path) -> Result<Self, VoxaError> {
        let (mut archive, _) = open_archive(ArchiveFile::open(path)?).map_err(archive_error)?;
        let mut files = Vec::new();
        for at in 0..archive.len() {
            let entry = archive.by_index_raw(at).map_err(archive_error)?;
            if entry.is_file() {
                let name = entry.name().map_err(archive_error)?;
                files.push((name.replace('\\', "/"), at));
            }
        }
        files.sort();
        Ok(Self {
            archive,
            files: files.into(),
        })
    }

    fn read_at(&mut self, at: usize) -> Result<Vec<u8>, VoxaError> {
        let mut entry = self.archive.by_index(at).map_err(archive_error)?;
        read_limited(&mut entry, MAX_ENTRY_BYTES)
    }

    /// Hand the entry at `at` to `visit` as `name`, refused first if it is too large to be
    /// read whole.
    fn visit_one(
        &mut self,
        name: &str,
        at: usize,
        visit: &mut dyn FnMut(&str, &mut dyn Read) -> Result<(), VoxaError>,
    ) -> Result<(), VoxaError> {
        let mut entry = self.archive.by_index(at).map_err(archive_error)?;
        within_limit(entry.size(), MAX_ENTRY_BYTES)?;
        visit(name, &mut entry)
    }
}

/// How an archive was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opening {
    /// As its end record says, with the archive starting where the file does: quick.
    AsItsEndSays,
    /// By searching the file for its directory, as the zip library does by default.
    BySearching,
}

/// The signature that starts an archive's end record.
const END_SIGNATURE: &[u8] = b"PK\x05\x06";
/// How far back from a file's end its end record is looked for: at least as far as the zip
/// library looks (the longest comment an archive can have, the record, and a little more).
const END_SEARCHED: u64 = 64 * 1024 + 1024;

/// Open the zip archive in `file`.
///
/// The zip library's default opening searches the whole file for the archive's directory,
/// reading every byte of it: seconds for an export of hundreds of megabytes, far longer when
/// the file is not in the system's cache. An archive that starts where its file does needs no
/// search, since its end record says where the directory is. Told so, the library looks there
/// and only there; and when the record it reads is the only one near the file's end, what it
/// finds is what its search would have found. Any other file, such as one with data before the
/// archive or with a second end record near its end, is searched as before.
fn open_archive(reader: ArchiveFile) -> ZipResult<(ZipArchive<ArchiveFile>, Opening)> {
    if one_end_record(&mut reader.clone()) {
        let at_the_start = Config {
            archive_offset: ArchiveOffset::Known(0),
        };
        if let Ok(archive) = ZipArchive::with_config(at_the_start, reader.clone()) {
            return Ok((archive, Opening::AsItsEndSays));
        }
    }
    ZipArchive::new(reader).map(|archive| (archive, Opening::BySearching))
}

/// Whether the end of `file` holds the signature of an end record exactly once, so that the
/// record the zip library takes is the only one it could. False when the end cannot be read.
fn one_end_record(file: &mut ArchiveFile) -> bool {
    let mut end = Vec::new();
    let read = file
        .seek(SeekFrom::End(0))
        .and_then(|len| file.seek(SeekFrom::Start(len.saturating_sub(END_SEARCHED))))
        .and_then(|_| file.read_to_end(&mut end));
    read.is_ok()
        && end
            .windows(END_SIGNATURE.len())
            .filter(|four| *four == END_SIGNATURE)
            .count()
            == 1
}

/// A zip library error as a message for the person using Voxa. Entry names are left out: they
/// can contain a patient's name.
fn archive_error(error: zip::result::ZipError) -> VoxaError {
    use zip::result::ZipError;
    match error {
        ZipError::Io(io) => VoxaError::Io(io),
        ZipError::UnsupportedArchive(what) => {
            VoxaError::Archive(format!("it uses a feature Voxa cannot read ({what})"))
        }
        ZipError::InvalidArchive(_) => VoxaError::Archive("it is damaged or incomplete".to_owned()),
        _ => VoxaError::Archive("it could not be opened".to_owned()),
    }
}

impl SeriesSource for ZipSource {
    fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VoxaError>> + '_> {
        let files = Arc::clone(&self.files);
        Box::new((0..files.len()).filter_map(move |i| {
            let (name, at) = files.get(i)?;
            let entry = self.read_at(*at).map(|bytes| Entry {
                name: name.clone(),
                bytes,
            });
            Some(entry)
        }))
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, VoxaError> {
        let found = self
            .files
            .binary_search_by(|(known, _)| known.as_str().cmp(name));
        let at = found
            .ok()
            .and_then(|i| self.files.get(i))
            .map(|(_, at)| *at)
            .ok_or_else(|| VoxaError::MissingEntry(name.to_owned()))?;
        self.read_at(at)
    }

    fn visit_files(
        &mut self,
        visit: &mut dyn FnMut(&str, &mut dyn Read) -> Result<(), VoxaError>,
    ) -> Result<(), VoxaError> {
        let files = Arc::clone(&self.files);
        for (name, at) in files.iter() {
            self.visit_one(name, *at, visit)?;
        }
        Ok(())
    }

    fn names(&mut self) -> Option<Vec<String>> {
        Some(self.files.iter().map(|(name, _)| name.clone()).collect())
    }

    fn visit_these(
        &mut self,
        names: &[String],
        visit: &mut dyn FnMut(&str, &mut dyn Read) -> Result<(), VoxaError>,
    ) -> Result<(), VoxaError> {
        let files = Arc::clone(&self.files);
        for name in names {
            let found = files.binary_search_by(|(known, _)| known.as_str().cmp(name));
            let (name, at) = found
                .ok()
                .and_then(|i| files.get(i))
                .ok_or_else(|| VoxaError::MissingEntry(name.clone()))?;
            self.visit_one(name, *at, visit)?;
        }
        Ok(())
    }

    fn another_reader(&self) -> Option<Box<dyn SeriesSource>> {
        Some(Box::new(Self {
            archive: self.archive.clone(),
            files: Arc::clone(&self.files),
        }))
    }
}

#[cfg(test)]
mod tests;
