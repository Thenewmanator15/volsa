//! Protocol Buffers' wire format, read field by field from a file without holding it: every
//! length is checked against what is left of the message around it before anything is read,
//! and what is not wanted is skipped, never read into memory.

use std::io::{BufReader, ErrorKind, Read, Seek};

use super::NetworkError;

/// The most bytes a number written seven bits at a time may take.
const MOST_VARINT_BYTES: u32 = 10;

/// The kinds of field the wire format has, by their numbers.
pub(super) const VARINT: u8 = 0;
/// A field that is a length and then that many bytes.
pub(super) const LENGTH: u8 = 2;

/// A file being read field by field, and how far into it the reading is.
pub(super) struct Wire<R> {
    inner: BufReader<R>,
    at: u64,
}

impl<R: Read + Seek> Wire<R> {
    /// Reading starts at the beginning of `inner`.
    pub(super) fn new(inner: R) -> Self {
        Self {
            inner: BufReader::new(inner),
            at: 0,
        }
    }

    /// How far into the file the reading is.
    pub(super) fn at(&self) -> u64 {
        self.at
    }

    /// One byte, if the message, which ends at `end`, has another.
    fn byte(&mut self, end: u64) -> Result<u8, NetworkError> {
        if self.at >= end {
            return Err(NetworkError::NotANetwork);
        }
        let mut byte = [0_u8; 1];
        loop {
            match self.inner.read(&mut byte) {
                Ok(1) => break,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                _ => return Err(NetworkError::NotANetwork),
            }
        }
        self.at += 1;
        let [value] = byte;
        Ok(value)
    }

    /// A number written seven bits at a time, lowest first.
    pub(super) fn varint(&mut self, end: u64) -> Result<u64, NetworkError> {
        let mut value: u64 = 0;
        for count in 0..MOST_VARINT_BYTES {
            let byte = self.byte(end)?;
            let bits = u64::from(byte & 0x7f);
            let shift = count * 7;
            value |= bits.checked_shl(shift).ok_or(NetworkError::NotANetwork)?;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(NetworkError::NotANetwork)
    }

    /// A field's number and its kind. A field numbered 0 does not exist.
    pub(super) fn tag(&mut self, end: u64) -> Result<(u64, u8), NetworkError> {
        let key = self.varint(end)?;
        let field = key >> 3;
        if field == 0 {
            return Err(NetworkError::NotANetwork);
        }
        let kind = u8::try_from(key & 7).map_err(|_| NetworkError::NotANetwork)?;
        Ok((field, kind))
    }

    /// The length of a field of the length kind, checked against the message's `end`: where
    /// the field ends.
    pub(super) fn length(&mut self, end: u64) -> Result<u64, NetworkError> {
        let length = self.varint(end)?;
        let ends = self
            .at
            .checked_add(length)
            .ok_or(NetworkError::NotANetwork)?;
        if ends > end {
            return Err(NetworkError::NotANetwork);
        }
        Ok(ends)
    }

    /// Moves on to `to`, which lies at or after where the reading is.
    pub(super) fn skip_to(&mut self, to: u64) -> Result<(), NetworkError> {
        let forward = to.checked_sub(self.at).ok_or(NetworkError::NotANetwork)?;
        let forward = i64::try_from(forward).map_err(|_| NetworkError::NotANetwork)?;
        self.inner
            .seek_relative(forward)
            .map_err(|_| NetworkError::NotANetwork)?;
        self.at = to;
        Ok(())
    }

    /// Passes over a field of this kind, unread.
    pub(super) fn skip(&mut self, kind: u8, end: u64) -> Result<(), NetworkError> {
        let fixed = |wire: &mut Self, bytes: u64| {
            let to = wire.at.checked_add(bytes).filter(|to| *to <= end);
            wire.skip_to(to.ok_or(NetworkError::NotANetwork)?)
        };
        match kind {
            VARINT => self.varint(end).map(|_| ()),
            1 => fixed(self, 8),
            LENGTH => {
                let to = self.length(end)?;
                self.skip_to(to)
            }
            5 => fixed(self, 4),
            _ => Err(NetworkError::NotANetwork),
        }
    }

    /// A field of text of the length kind, if it is no longer than `most` bytes; `None`, and
    /// the field passed over, when it is longer. Text that is not UTF-8 is not ONNX.
    pub(super) fn text(&mut self, end: u64, most: u64) -> Result<Option<String>, NetworkError> {
        let to = self.length(end)?;
        let length = to - self.at;
        if length > most {
            self.skip_to(to)?;
            return Ok(None);
        }
        let length = usize::try_from(length).map_err(|_| NetworkError::NotANetwork)?;
        let mut bytes = vec![0_u8; length];
        self.inner
            .read_exact(&mut bytes)
            .map_err(|_| NetworkError::NotANetwork)?;
        self.at = to;
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| NetworkError::NotANetwork)
    }
}
