//! One message on a pipe: a byte for its kind, its length as eight little-endian bytes, and
//! that many bytes. A length over 1 GiB is refused before anything of it is read.

use std::io::{self, ErrorKind, Read, Write};

/// A job, as JSON: Voxa to the helper.
pub(crate) const OPEN: u8 = 1;
/// A block of the scan, as 32-bit numbers: Voxa to the helper.
pub(crate) const BLOCK: u8 = 2;
/// The last block has been sent: Voxa to the helper.
pub(crate) const FINISH: u8 = 3;
/// The network is open: the helper to Voxa.
pub(crate) const OPENED: u8 = 11;
/// A block's labels, as 16-bit numbers: the helper to Voxa.
pub(crate) const LABELS: u8 = 12;
/// The run is over, with what the helper held of a card, as JSON: the helper to Voxa.
pub(crate) const DONE: u8 = 13;
/// Why there is no answer, as JSON: the helper to Voxa.
pub(crate) const FAILED: u8 = 14;

/// The longest a message may be: 1 GiB.
const MOST: u64 = 1 << 30;

/// Why a message could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameError {
    /// Its length is over 1 GiB.
    TooLong,
    /// The pipe ended part-way through it, or could not be read.
    Cut,
    /// There was not the memory for it.
    NoRoom,
}

/// Writes one message, and sends it on at once.
pub(crate) fn write_frame(out: &mut impl Write, kind: u8, payload: &[u8]) -> io::Result<()> {
    out.write_all(&[kind])?;
    out.write_all(&(payload.len() as u64).to_le_bytes())?;
    out.write_all(payload)?;
    out.flush()
}

/// Reads one message: its kind and its bytes. `None` when the pipe ended before a message
/// began.
pub(crate) fn read_frame(input: &mut impl Read) -> Result<Option<(u8, Vec<u8>)>, FrameError> {
    let mut kind = [0_u8; 1];
    loop {
        match input.read(&mut kind) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Err(FrameError::Cut),
        }
    }
    let mut length = [0_u8; 8];
    input.read_exact(&mut length).map_err(|_| FrameError::Cut)?;
    let length = u64::from_le_bytes(length);
    if length > MOST {
        return Err(FrameError::TooLong);
    }
    let count = usize::try_from(length).map_err(|_| FrameError::TooLong)?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(count)
        .map_err(|_| FrameError::NoRoom)?;
    input
        .take(length)
        .read_to_end(&mut payload)
        .map_err(|_| FrameError::Cut)?;
    if payload.len() != count {
        return Err(FrameError::Cut);
    }
    let [kind] = kind;
    Ok(Some((kind, payload)))
}

/// 32-bit numbers as little-endian bytes.
pub(crate) fn from_floats(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Little-endian bytes as 32-bit numbers, the memory for them asked for in a way that can be
/// refused.
///
/// # Errors
///
/// [`FrameError::Cut`] when they are not a whole number of them, [`FrameError::NoRoom`] when
/// there is not the memory.
pub(crate) fn to_floats(bytes: &[u8]) -> Result<Vec<f32>, FrameError> {
    let (numbers, rest) = bytes.as_chunks::<4>();
    if !rest.is_empty() {
        return Err(FrameError::Cut);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(numbers.len())
        .map_err(|_| FrameError::NoRoom)?;
    values.extend(numbers.iter().map(|four| f32::from_le_bytes(*four)));
    Ok(values)
}

/// 16-bit numbers as little-endian bytes.
pub(crate) fn from_labels(labels: &[u16]) -> Vec<u8> {
    labels
        .iter()
        .flat_map(|label| label.to_le_bytes())
        .collect()
}

/// Little-endian bytes as 16-bit numbers; `None` when they are not a whole number of them.
pub(crate) fn to_labels(bytes: &[u8]) -> Option<Vec<u16>> {
    let (numbers, rest) = bytes.as_chunks::<2>();
    rest.is_empty()
        .then(|| numbers.iter().map(|two| u16::from_le_bytes(*two)).collect())
}
