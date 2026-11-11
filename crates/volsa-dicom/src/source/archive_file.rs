//! An archive file that several readers can share, each reading where it likes.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;

/// How much is read at once for a smaller read, so that the many small reads of a zip's
/// directory do not each go to the file.
const READ_AHEAD: usize = 8 * 1024;

/// An open file read at a position of its own. Clones share the open file but not the
/// position, so each can be read on its own thread without moving the others.
pub(super) struct ArchiveFile {
    file: Arc<File>,
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

impl Clone for ArchiveFile {
    fn clone(&self) -> Self {
        Self {
            file: Arc::clone(&self.file),
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
