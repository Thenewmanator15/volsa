//! Reading only as much of a file as its header needs.

use std::cell::Cell;
use std::io::{self, Read};

use voxa_core::VoxaError;

use super::{Header, read_header_start};
use crate::source::within_limit;

/// How much of a file is read first: enough for the header of nearly every single-frame file.
const FIRST_READ: u64 = 64 * 1024;

/// The header of the file `contents` reads, reading as little of it as the header needs: its
/// first 64 KiB, and twice as much again each time that is too little to be sure of the
/// header. What it gives is what parsing the whole file gives. No more than `limit` bytes are
/// read.
///
/// With the header, which is `None` for a file that is not an image, it gives the start of the
/// file that was read to find it. Of a file that is not an image but is DICOM, that is the
/// whole file, which is what a document is read from.
///
/// # Errors
/// An error reading the file, or [`VoxaError::Archive`] when more than `limit` bytes would
/// have to be read.
pub(crate) fn read_header_from(
    contents: &mut dyn Read,
    limit: u64,
) -> Result<(Option<Header>, Vec<u8>), VoxaError> {
    let mut start = Vec::new();
    let mut wanted = FIRST_READ;
    loop {
        let whole = read_more(contents, &mut start, wanted, limit)?;
        if let Ok(header) = read_header_start(&start, whole) {
            return Ok((header, start));
        }
        wanted = wanted.saturating_mul(2);
    }
}

/// Read on from `contents` until `start` holds `wanted` bytes or the file has ended, refusing
/// to hold more than `limit`. True when the file has ended.
fn read_more(
    contents: &mut dyn Read,
    start: &mut Vec<u8>,
    wanted: u64,
    limit: u64,
) -> Result<bool, VoxaError> {
    let wanted = wanted.min(limit.saturating_add(1));
    let missing = wanted.saturating_sub(start.len() as u64);
    start.reserve(usize::try_from(missing.min(FIRST_READ * 16)).unwrap_or(0));
    let read = contents.take(missing).read_to_end(start)?;
    within_limit(start.len() as u64, limit)?;
    Ok((read as u64) < missing)
}

/// The start of a file, read as the file itself would be read, noting whether any read asked
/// for more than the start holds. Until one does, every read gives exactly what the same read
/// of the whole file gives, so a parse that never ran short is the whole file's parse.
pub(super) struct Watched<'a> {
    rest: &'a [u8],
    short: &'a Cell<bool>,
}

impl<'a> Watched<'a> {
    pub(super) fn new(start: &'a [u8], short: &'a Cell<bool>) -> Self {
        Self { rest: start, short }
    }
}

impl Read for Watched<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.len() > self.rest.len() {
            self.short.set(true);
        }
        let count = out.len().min(self.rest.len());
        if let (Some((given, rest)), Some(target)) =
            (self.rest.split_at_checked(count), out.get_mut(..count))
        {
            target.copy_from_slice(given);
            self.rest = rest;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests;
