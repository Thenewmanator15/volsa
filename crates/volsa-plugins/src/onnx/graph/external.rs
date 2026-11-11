//! Where a tensor kept outside the network file says its numbers are, and whether that is a
//! part of a file the plugin's description lists.

use std::io::{Read, Seek};

use super::super::NetworkError;
use super::super::wire::{LENGTH, Wire};
use super::MOST_TEXT;
use crate::Listed;

/// Where a tensor kept outside the file says its numbers are.
#[derive(Default)]
pub(super) struct External {
    /// Whether any entry was read.
    pub(super) seen: bool,
    location: Option<String>,
    offset: Option<String>,
    length: Option<String>,
}

impl External {
    /// Reads one entry: a key and its value.
    pub(super) fn read<R: Read + Seek>(
        &mut self,
        wire: &mut Wire<R>,
        end: u64,
    ) -> Result<(), NetworkError> {
        self.seen = true;
        let (mut key, mut value) = (None, None);
        while wire.at() < end {
            match wire.tag(end)? {
                (1, LENGTH) => key = wire.text(end, MOST_TEXT)?,
                (2, LENGTH) => value = wire.text(end, MOST_TEXT)?,
                (_, kind) => wire.skip(kind, end)?,
            }
        }
        match key.as_deref() {
            Some("location") => self.location = value,
            Some("offset") => self.offset = value,
            Some("length") => self.length = value,
            _ => {}
        }
        Ok(())
    }

    /// Whether the numbers lie inside a listed file: its name plain and listed, and the offset
    /// and the length within it.
    pub(super) fn within(&self, listed: &[Listed]) -> bool {
        let Some(location) = self.location.as_deref() else {
            return false;
        };
        let Some(file) = listed.iter().find(|file| file.name == location) else {
            return false;
        };
        let number = |text: &Option<String>| text.as_deref().map(str::parse::<u64>);
        let offset = match number(&self.offset) {
            None => 0,
            Some(Ok(offset)) => offset,
            Some(Err(_)) => return false,
        };
        let length = match number(&self.length) {
            None => file.bytes.checked_sub(offset),
            Some(Ok(length)) => Some(length),
            Some(Err(_)) => return false,
        };
        volsa_models::is_plain_name(location)
            && length
                .and_then(|length| offset.checked_add(length))
                .is_some_and(|ends| ends <= file.bytes)
    }
}
