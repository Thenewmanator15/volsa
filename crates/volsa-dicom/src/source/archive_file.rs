//! An archive file that several readers can share, each reading where it likes.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;

/// How much is read at once for a smaller read, so that the many small reads of a zip's
/// directory, and of a compressed entry as it is inflated, do not each go to the disk.
const READ_AHEAD: usize = 64 * 1024;

/// An open file read at a position of its own. A clone has its own position and, where the
/// file was opened by its path, its own handle of the file: Windows takes the reads made
/// through one handle in turn, which is quick from the file cache and slow from the disk.
pub(super) struct ArchiveFile {
    file: Arc<File>,
    /// Where the file is, for a clone to open it again.
    path: Option<Arc<Path>>,
    /// Where the next read starts.
    at: u64,
    /// Bytes read ahead, which start at `ahead_at` in the file.
    ahead: Vec<u8>,
    ahead_at: u64,
}

impl ArchiveFile {
    pub(super) fn new(file: File) -> Self {
        Self {
            file: Arc::new(file),
            path: None,
            at: 0,
            ahead: Vec::new(),
            ahead_at: 0,
        }
    }

    /// The bytes read ahead from the position on, if they hold all of the `wanted` bytes.
    fn held(&self, wanted: usize) -> Option<&[u8]> {
        let from = usize::try_from(self.at.checked_sub(self.ahead_at)?).ok()?;
        self.ahead.get(from..).filter(|rest| rest.len() >= wanted)
    }
}

impl ArchiveFile {
    /// The file at `path`, whose clones each open it again.
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        Ok(Self {
            path: Some(Arc::from(path)),
            ..Self::new(File::open(path)?)
        })
    }
}

impl Clone for ArchiveFile {
    fn clone(&self) -> Self {
        // A handle of its own where the file can be opened again; the same one where not.
        let own = self.path.as_deref().and_then(|path| File::open(path).ok());
        Self {
            file: own.map_or_else(|| Arc::clone(&self.file), Arc::new),
            path: self.path.clone(),
            at: self.at,
            ahead: Vec::new(),
            ahead_at: 0,
        }
    }
}

impl Read for ArchiveFile {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let count = if let Some(held) = self.held(out.len()) {
            copy_start(held, out)
        } else if out.len() >= READ_AHEAD {
            read_at(&self.file, out, self.at)?
        } else {
            self.ahead.resize(READ_AHEAD, 0);
            let filled = read_at(&self.file, &mut self.ahead, self.at)?;
            self.ahead.truncate(filled);
            self.ahead_at = self.at;
            copy_start(&self.ahead, out)
        };
        self.at += count as u64;
        Ok(count)
    }
}

impl Seek for ArchiveFile {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::End(by) => self.file.metadata()?.len().checked_add_signed(by),
            SeekFrom::Current(by) => self.at.checked_add_signed(by),
        };
        self.at = target.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "a move to before the file's start",
            )
        })?;
        Ok(self.at)
    }
}

/// Copy as much of `from` as fits into the start of `to`, and say how much that was.
fn copy_start(from: &[u8], to: &mut [u8]) -> usize {
    let count = from.len().min(to.len());
    if let (Some(source), Some(target)) = (from.get(..count), to.get_mut(..count)) {
        target.copy_from_slice(source);
    }
    count
}

/// Read into `out` from `at` in `file`, leaving no position behind that another read relies on.
#[cfg(windows)]
fn read_at(file: &File, out: &mut [u8], at: u64) -> io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, out, at)
}

/// Read into `out` from `at` in `file`, leaving no position behind that another read relies on.
#[cfg(unix)]
fn read_at(file: &File, out: &mut [u8], at: u64) -> io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, out, at)
}

#[cfg(test)]
mod tests;
