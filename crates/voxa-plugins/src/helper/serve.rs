//! The helper's side: one job served over its input and output, then the end.

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};
use voxa_models::{Network, Stop, Tensor, file_sum};

use super::frame::{
    BLOCK, DONE, FAILED, FINISH, FrameError, LABELS, OPEN, OPENED, from_labels, read_frame,
    to_floats, write_frame,
};
use super::{Failure, OpenJob, Opener};
use crate::Layout;
use crate::finish::{FinishError, labels_of};

/// The most a network file is read for its sum: 2 GiB, the most one may be.
const MOST_NETWORK: u64 = 1 << 31;

/// What the helper says when the run is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Done {
    /// How many bytes of the card it held with the network open, if it ran on one.
    pub(crate) card_bytes: Option<u64>,
}

/// Why the helper stopped serving: Voxa spoke out of turn, or the pipe broke. Voxa sees the
/// helper end without an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ServeError(&'static str);

/// Serves one job: waits for it, checks the network file's sum, loads the runtime and opens the
/// network, then answers each block with its labels until Voxa says the last was sent or the
/// input ends. A failure is told to Voxa as a message, and the helper then ends.
///
/// # Errors
///
/// [`ServeError`] when Voxa's messages are not a job and its blocks, or a pipe breaks.
pub fn serve(
    input: &mut impl Read,
    output: &mut impl Write,
    opener: &dyn Opener,
) -> Result<(), ServeError> {
    let job = match read_frame(input) {
        Ok(Some((OPEN, payload))) => serde_json::from_slice::<OpenJob>(&payload)
            .map_err(|_| ServeError("the job could not be read"))?,
        Ok(None) => return Ok(()),
        _ => return Err(ServeError("the first message was not a job")),
    };
    let sum = file_sum(&job.network, MOST_NETWORK);
    if !sum.is_ok_and(|sum| sum.eq_ignore_ascii_case(&job.sha256)) {
        return failed(output, &Failure::Changed);
    }
    if let Err(why) = opener.load(&job.runtime) {
        return failed(output, &Failure::Failed(why.to_string()));
    }
    let stop = Stop::new();
    let mut network = match opener.open(&job, &stop) {
        Ok(network) => network,
        Err(why) => return failed(output, &Failure::from(why)),
    };
    send(output, OPENED, &[])?;
    loop {
        match read_frame(input) {
            Ok(None) => return Ok(()),
            Ok(Some((BLOCK, payload))) => {
                let values = match to_floats(&payload) {
                    Ok(values) => values,
                    Err(FrameError::NoRoom) => return failed(output, &Failure::NoRoom),
                    Err(_) => return Err(ServeError("a block was not numbers")),
                };
                drop(payload);
                match answer(network.as_mut(), &job, values) {
                    Ok(labels) => send(output, LABELS, &from_labels(&labels))?,
                    Err(failure) => return failed(output, &failure),
                }
            }
            Ok(Some((FINISH, _))) => {
                let done = Done {
                    card_bytes: opener.card_in_use(&job.device),
                };
                drop(network);
                let report = serde_json::to_vec(&done).map_err(|_| ServeError("a report"))?;
                return send(output, DONE, &report);
            }
            Err(FrameError::NoRoom) => return failed(output, &Failure::NoRoom),
            _ => return Err(ServeError("a message out of turn")),
        }
    }
}

/// The labels of one block: the network run on it, and its answer finished.
fn answer(network: &mut dyn Network, job: &OpenJob, values: Vec<f32>) -> Result<Vec<u16>, Failure> {
    let [a, b, c] = job.block;
    let shape = match job.layout {
        Layout::ChannelsFirst => vec![1, 1, a, b, c],
        Layout::ChannelsLast => vec![1, a, b, c, 1],
    };
    let wrong = || Failure::Answer(FinishError::WrongAnswer);
    let tensor = Tensor::f32(shape, values).ok_or_else(wrong)?;
    let answers = network.run(vec![tensor]).map_err(Failure::from)?;
    let first = answers.into_iter().next().ok_or_else(wrong)?;
    labels_of(first, job.layout, job.block, &job.finish).map_err(Failure::Answer)
}

/// Sends one message.
fn send(output: &mut impl Write, kind: u8, payload: &[u8]) -> Result<(), ServeError> {
    write_frame(output, kind, payload).map_err(|_| ServeError("the pipe to Voxa broke"))
}

/// Tells Voxa why there is no answer; the helper then ends.
fn failed(output: &mut impl Write, failure: &Failure) -> Result<(), ServeError> {
    let report = serde_json::to_vec(failure).map_err(|_| ServeError("a report"))?;
    send(output, FAILED, &report)
}
