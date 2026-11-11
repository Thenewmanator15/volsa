//! Volsa's side: the job sent, each block sent and its labels read back, and the end. A helper
//! that ends, or whose pipes close, before it answers is [`HelperError::Ended`].

use std::io::{Read, Write};

use super::frame::{
    BLOCK, DONE, FAILED, FINISH, LABELS, OPEN, OPENED, from_floats, read_frame, to_labels,
    write_frame,
};
use super::serve::Done;
use super::{Failure, HelperError, OpenJob};

/// Volsa's end of the pipes to one helper.
#[derive(Debug)]
pub struct Client<R, W> {
    read: R,
    write: W,
}

impl<R: Read, W: Write> Client<R, W> {
    /// Speaks to a helper that reads `write` and writes `read`.
    pub fn new(read: R, write: W) -> Self {
        Self { read, write }
    }

    /// Sends the job, and waits until the network is open.
    ///
    /// # Errors
    ///
    /// [`HelperError`] for why the network was not opened.
    pub fn open(&mut self, job: &OpenJob) -> Result<(), HelperError> {
        let payload = serde_json::to_vec(job).map_err(|_| HelperError::Ended)?;
        self.send(OPEN, &payload)?;
        self.reply(OPENED).map(|_| ())
    }

    /// Sends one block of the scan, and gives its labels.
    ///
    /// # Errors
    ///
    /// [`HelperError`] for why there are no labels.
    pub fn block(&mut self, values: &[f32]) -> Result<Vec<u16>, HelperError> {
        self.send(BLOCK, &from_floats(values))?;
        let payload = self.reply(LABELS)?;
        to_labels(&payload).ok_or(HelperError::Ended)
    }

    /// Says the last block was sent, and gives how many bytes of the card the helper held, if
    /// it ran on one.
    ///
    /// # Errors
    ///
    /// [`HelperError`] when the helper did not end the run as it should.
    pub fn finish(&mut self) -> Result<Option<u64>, HelperError> {
        self.send(FINISH, &[])?;
        let payload = self.reply(DONE)?;
        let done: Done = serde_json::from_slice(&payload).map_err(|_| HelperError::Ended)?;
        Ok(done.card_bytes)
    }

    /// Sends one message. A pipe that will not take it is a helper that has ended, perhaps
    /// after saying why: a helper refused the memory for a block says so while the block is
    /// still being sent, and ends. What it said is read before the end is taken as silent.
    fn send(&mut self, kind: u8, payload: &[u8]) -> Result<(), HelperError> {
        if write_frame(&mut self.write, kind, payload).is_ok() {
            return Ok(());
        }
        match read_frame(&mut self.read) {
            Ok(Some((FAILED, payload))) => {
                let failure: Failure =
                    serde_json::from_slice(&payload).map_err(|_| HelperError::Ended)?;
                Err(failure.into())
            }
            _ => Err(HelperError::Ended),
        }
    }

    /// Reads the reply, which must be of `kind` or say why there is none.
    fn reply(&mut self, kind: u8) -> Result<Vec<u8>, HelperError> {
        match read_frame(&mut self.read) {
            Ok(Some((got, payload))) if got == kind => Ok(payload),
            Ok(Some((FAILED, payload))) => {
                let failure: Failure =
                    serde_json::from_slice(&payload).map_err(|_| HelperError::Ended)?;
                Err(failure.into())
            }
            _ => Err(HelperError::Ended),
        }
    }
}
