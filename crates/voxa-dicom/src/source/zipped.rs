//! The files inside a zip archive, read straight from the archive.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use voxa_core::VoxaError;

use super::{Entry, MAX_ENTRY_BYTES, SeriesSource, read_limited};

/// The files inside a zip archive, read straight from the archive.
pub(super) struct ZipSource {
    archive: zip::ZipArchive<BufReader<File>>,
    /// (name with `/`, position in the archive) of every file, sorted by name.
    files: Vec<(String, usize)>,
}

impl ZipSource {
    pub(super) fn open(path: &Path) -> Result<Self, VoxaError> {
        let mut archive =
            zip::ZipArchive::new(BufReader::new(File::open(path)?)).map_err(archive_error)?;
        let mut files = Vec::new();
        for at in 0..archive.len() {
            let entry = archive.by_index_raw(at).map_err(archive_error)?;
            if entry.is_file() {
                let name = entry.name().map_err(archive_error)?;
                files.push((name.replace('\\', "/"), at));
            }
        }
        files.sort();
        Ok(Self { archive, files })
    }

    fn read_at(&mut self, at: usize) -> Result<Vec<u8>, VoxaError> {
        let mut entry = self.archive.by_index(at).map_err(archive_error)?;
        read_limited(&mut entry, MAX_ENTRY_BYTES)
    }
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
        let files = self.files.clone();
        Box::new(files.into_iter().map(|(name, at)| {
            let bytes = self.read_at(at)?;
            Ok(Entry { name, bytes })
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
}
