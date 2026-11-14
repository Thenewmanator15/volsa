//! Refusing a file whose sequences nest too deeply to read safely, before the DICOM library
//! builds it.
//!
//! The library builds a sequence inside another by calling itself once more, with no limit of
//! its own, so a file nested deeply enough uses up the stack of the thread reading it. That
//! stops the whole program at once: it is not a panic, and nothing can catch it. A level of
//! nesting takes about 20 bytes of file and, measured, about 1.2 KB of stack in a release build
//! (7.6 KB in a debug build), so a file of a few tens of kilobytes was enough.
//!
//! So each place Volsa gives the library bytes first walks the same bytes here, with the
//! library's own reader of one token at a time, which keeps its place on the heap rather than the
//! stack, and refuses the file when its sequences nest deeper than [`MAX_NESTING`]. The walk is
//! set up as the library sets up its own read (the preamble, the file header, the transfer
//! syntax, a deflated data set inflated) and reads the very tokens the library builds from, so
//! it meets every sequence the library would.

use std::io::{self, Cursor, Read};

use dicom_core::Tag;
use dicom_dictionary_std::tags;
use dicom_encoding::{Codec, TransferSyntax};
use dicom_object::FileMetaTable;
use dicom_parser::dataset::read::{DataSetReaderOptions, ValueReadStrategy};
use dicom_parser::dataset::{DataSetReader, DataToken};
use dicom_transfer_syntax_registry::{TransferSyntaxIndex, TransferSyntaxRegistry};

/// The deepest one sequence may be nested in others in a file Volsa reads: a sequence in the
/// data set itself is 1 deep.
///
/// The deepest nesting in real files is in structured reports, whose tree of findings puts a
/// content sequence at each level, each item holding a code sequence or two more: some ten to
/// twenty sequences deep. 64 is several times that, and costs at most about 0.5 MB of stack in a
/// debug build and 80 KB in a release build: within even the 1 MiB a Windows program's main
/// thread has.
pub const MAX_NESTING: usize = 64;

/// The stack each thread that reads DICOM files is given: 16 MiB, eight times what a thread
/// gets by default.
///
/// With nesting checked, reading a file takes at most about 0.5 MB of it. The rest is a second
/// line of defence: should any read ever miss the check, a file would have to be eight times
/// deeper to stop Volsa, and the decoders, which are other people's code too, have room. Only
/// address space is set aside; the system gives a stack memory as it is used.
pub const READING_STACK_BYTES: usize = 16 * 1024 * 1024;

/// Why a file nested too deeply is not read, in the words the person using Volsa sees.
pub(crate) const TOO_DEEP: &str = "nested too deeply to read safely";

/// A value at the top of the data set this long or longer, such as the pixels, is passed over
/// rather than read, so that checking a whole file does not copy its pixels.
const PASS_OVER: u32 = 64 * 1024;

/// A file's sequences nest deeper than [`MAX_NESTING`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TooDeep;

/// An entry of a source that is refused unread, and why: what the index lists for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// Name of the entry in the source.
    pub entry: String,
    /// Why it is not read, in a few plain words: "nested too deeply to read safely".
    pub reason: String,
}

/// Refuse `bytes` when the sequences in them nest deeper than [`MAX_NESTING`]; to be called
/// before the bytes are given to the DICOM library. `read_until` is the tag the library is told
/// to read until, as `OpenFileOptions::read_until` takes it, and `None` when it reads the whole
/// file.
///
/// Bytes the library cannot read, or can read only so far, are walked as far as it would read
/// them and are not refused for that: the library refuses them itself, having built no deeper
/// than the walk went.
pub(crate) fn check_nesting(bytes: &[u8], read_until: Option<Tag>) -> Result<(), TooDeep> {
    let mut file = Cursor::new(bytes);
    file.set_position(preamble(bytes));
    let Ok(meta) = FileMetaTable::from_reader(&mut file) else {
        return Ok(());
    };
    let Some(syntax) = TransferSyntaxRegistry.get(meta.transfer_syntax()) else {
        return Ok(());
    };
    match syntax.codec() {
        Codec::Dataset(Some(adapter)) => {
            walk(adapter.adapt_reader(Box::new(file)), syntax, read_until)
        }
        Codec::Dataset(None) => Ok(()),
        Codec::None | Codec::EncapsulatedPixelData(..) => walk(file, syntax, read_until),
    }
}

/// How many bytes come before the file header: a 128-byte preamble when the magic code follows
/// it, and otherwise none, as the library decides.
pub(crate) fn preamble(bytes: &[u8]) -> u64 {
    if bytes.get(128..132) == Some(b"DICM".as_slice()) {
        128
    } else {
        0
    }
}

/// Walk the data set `from` gives, written in `syntax`, as far as the library would read it,
/// counting how deeply its sequences nest.
fn walk(
    mut from: impl Read,
    syntax: &TransferSyntax,
    read_until: Option<Tag>,
) -> Result<(), TooDeep> {
    // Values as they are stored: the walk needs only where each element starts and ends, and
    // text that is not decoded cannot fail to decode where the library's reading goes on.
    let options = DataSetReaderOptions::default().value_read(ValueReadStrategy::Raw);
    let mut depth: usize = 0;
    loop {
        let Ok(tokens) = DataSetReader::new_with_ts_options(&mut from, syntax, options) else {
            return Ok(());
        };
        let mut pass_over = None;
        for token in tokens {
            let Ok(token) = token else {
                return Ok(());
            };
            // The tag met, whether it opens a sequence, and the length of a plain value.
            let (tag, opens, len) = match token {
                DataToken::SequenceStart { tag, .. } => (tag, true, 0),
                DataToken::PixelSequenceStart => (tags::PIXEL_DATA, true, 0),
                DataToken::ElementHeader(header) => (header.tag, false, header.len.0),
                DataToken::SequenceEnd => {
                    depth = depth.saturating_sub(1);
                    continue;
                }
                _ => continue,
            };
            if depth == 0 && read_until.is_some_and(|until| until <= tag) {
                return Ok(());
            }
            if opens {
                depth += 1;
                if depth > MAX_NESTING {
                    return Err(TooDeep);
                }
            } else if depth == 0 && len >= PASS_OVER {
                // At the top of the data set nothing is open, so the walk can stop, pass over
                // the value, and begin again after it.
                pass_over = Some(u64::from(len));
                break;
            }
        }
        let Some(len) = pass_over else {
            return Ok(());
        };
        let passed = io::copy(&mut (&mut from).take(len), &mut io::sink());
        if passed.ok() != Some(len) {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests;
