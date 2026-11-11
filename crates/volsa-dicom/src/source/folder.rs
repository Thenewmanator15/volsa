//! Files under a folder on disk, or one file on its own.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use voxa_core::VoxaError;

use super::{Entry, MAX_ENTRY_BYTES, SeriesSource, read_limited, within_limit};

/// Files under a folder on disk, or one file on its own.
pub(super) struct FolderSource {
    /// (name relative to the root with `/`, path on disk), sorted by name. Shared with the
    /// source's other readers.
    files: Arc<[(String, PathBuf)]>,
}

impl FolderSource {
    pub(super) fn walk(root: &Path) -> Result<Self, VoxaError> {
        let mut files = Vec::new();
        collect_files(root, "", &mut files)?;
        files.sort();
        Ok(Self {
            files: files.into(),
        })
    }

    pub(super) fn single(path: &Path) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self {
            files: Arc::new([(name, path.to_path_buf())]),
        }
    }
}

/// Add every file under `folder` to `files`. Links to folders are not followed, so a loop of
/// links cannot trap the walk.
fn collect_files(
    folder: &Path,
    prefix: &str,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), VoxaError> {
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            collect_files(&entry.path(), &format!("{name}/"), files)?;
        } else if kind.is_file() || (kind.is_symlink() && entry.path().is_file()) {
            files.push((name, entry.path()));
        }
    }
    Ok(())
}

impl SeriesSource for FolderSource {
    fn entries(&mut self) -> Box<dyn Iterator<Item = Result<Entry, VoxaError>> + '_> {
        Box::new(self.files.iter().map(|(name, path)| {
            let bytes = read_limited(&mut File::open(path)?, MAX_ENTRY_BYTES)?;
            Ok(Entry {
                name: name.clone(),
                bytes,
            })
        }))
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>, VoxaError> {
        let found = self
            .files
            .binary_search_by(|(known, _)| known.as_str().cmp(name));
        let path = found
            .ok()
            .and_then(|at| self.files.get(at))
            .map(|(_, path)| path)
            .ok_or_else(|| VoxaError::MissingEntry(name.to_owned()))?;
        read_limited(&mut File::open(path)?, MAX_ENTRY_BYTES)
    }

    fn visit_files(
        &mut self,
        visit: &mut dyn FnMut(&str, &mut dyn Read) -> Result<(), VoxaError>,
    ) -> Result<(), VoxaError> {
        for (name, path) in self.files.iter() {
            let mut file = File::open(path)?;
            within_limit(file.metadata()?.len(), MAX_ENTRY_BYTES)?;
            visit(name, &mut file)?;
        }
        Ok(())
    }

    fn another_reader(&self) -> Option<Box<dyn SeriesSource>> {
        Some(Box::new(Self {
            files: Arc::clone(&self.files),
        }))
    }
}
