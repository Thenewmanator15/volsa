//! Volsa and a helper over a pair of pipes: a job opened, blocks answered with labels, and every
//! way it can go wrong coming back as a failure, never a hang.

use std::io::{PipeReader, PipeWriter};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use volsa_models::testkit::Canned;
use volsa_models::{Network, Numbers, RunError, RuntimeError, Stop, Tensor};

use super::frame::{read_frame, write_frame};
use super::{Client, HelperError, OpenJob, Opener, Where, serve};
use crate::finish::FinishError;
use crate::testkit::sum;
use crate::{Gives, LabelOutput, Layout, Name, Side};

/// What a stand-in network's answer to a block is.
#[derive(Clone, Copy)]
enum Answers {
    /// Two channels: 0.5, and the value given; the label is 1 where the value is above 0.5.
    Halves,
    /// A number that is not finite.
    NotFinite,
}

/// A stand-in for the runtime: it opens a stand-in network, or fails as told.
struct StandIn {
    answers: Answers,
    refuse: Option<RunError>,
    opened: Arc<AtomicBool>,
}

impl Opener for StandIn {
    fn load(&self, _runtime: &Path) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn open(&self, _job: &OpenJob, _stop: &Stop) -> Result<Box<dyn Network>, RunError> {
        self.opened.store(true, Ordering::SeqCst);
        if let Some(why) = self.refuse.clone() {
            return Err(why);
        }
        let answers = self.answers;
        Ok(Box::new(Canned::new(move |inputs: &[Tensor]| {
            let Numbers::F32(values) = &inputs[0].numbers else {
                return Err(RunError::Failed("floats were expected".to_owned()));
            };
            let mut shape = inputs[0].shape.clone();
            shape[1] = 2;
            let mut numbers = vec![0.5; values.len()];
            numbers.extend_from_slice(values);
            if let Answers::NotFinite = answers {
                numbers[0] = f32::NAN;
            }
            Ok(vec![Tensor {
                shape,
                numbers: Numbers::F32(numbers),
            }])
        })))
    }

    fn card_in_use(&self, _place: &Where) -> Option<u64> {
        Some(123)
    }
}

/// The job for a network file of these bytes, held in `folder`, its sum `sha256`.
fn job(folder: &Path, bytes: &[u8], sha256: Option<String>) -> OpenJob {
    let network = folder.join("net.onnx");
    std::fs::write(&network, bytes).expect("written");
    let name = Name {
        id: 1,
        name: "Bright".to_owned(),
        side: Side::Midline,
        group: String::new(),
        colour: None,
    };
    OpenJob {
        runtime: folder.to_owned(),
        network,
        sha256: sha256.unwrap_or_else(|| sum(bytes)),
        device: Where::Processor,
        free: vec![("x".to_owned(), 2)],
        output: None,
        layout: Layout::ChannelsFirst,
        block: [2, 1, 1],
        finish: LabelOutput {
            gives: Gives::Scores {
                channels: vec![0, 1],
            },
            from: None,
            keep_largest_piece: false,
            names: vec![name],
        },
    }
}

/// A client talking to `serve` with `opener` on a thread of its own.
fn served(opener: StandIn) -> (Client<PipeReader, PipeWriter>, JoinHandle<()>) {
    let (mut from_volsa, to_helper) = std::io::pipe().expect("a pipe");
    let (from_helper, mut to_volsa) = std::io::pipe().expect("a pipe");
    let serving = thread::spawn(move || {
        let _ = serve(&mut from_volsa, &mut to_volsa, &opener);
    });
    (Client::new(from_helper, to_helper), serving)
}

/// A stand-in that opens the halves network.
fn halves() -> StandIn {
    StandIn {
        answers: Answers::Halves,
        refuse: None,
        opened: Arc::default(),
    }
}

#[test]
fn a_job_is_opened_and_each_block_answered_with_labels() {
    let folder = tempfile::tempdir().expect("a folder");
    let (mut client, serving) = served(halves());
    client
        .open(&job(folder.path(), b"net", None))
        .expect("opened");
    assert_eq!(client.block(&[0.9, 0.1]).expect("labels"), [1, 0]);
    assert_eq!(client.block(&[0.2, 0.7]).expect("labels"), [0, 1]);
    assert_eq!(client.finish().expect("done"), Some(123));
    drop(client);
    serving.join().expect("the helper ended");
}

#[test]
fn a_changed_network_file_is_refused_before_it_is_opened() {
    let folder = tempfile::tempdir().expect("a folder");
    let stand_in = halves();
    let opened = Arc::clone(&stand_in.opened);
    let (mut client, _serving) = served(stand_in);
    let wrong = Some(sum(b"another network"));
    let refused = client
        .open(&job(folder.path(), b"net", wrong))
        .expect_err("changed");
    assert_eq!(refused, HelperError::Changed);
    let words = "This plugin's files have changed since it was added. Remove it and add it again.";
    assert_eq!(refused.to_string(), words);
    assert!(!opened.load(Ordering::SeqCst));
}

#[test]
fn no_room_on_the_card_comes_back_as_no_room() {
    let folder = tempfile::tempdir().expect("a folder");
    let opener = StandIn {
        refuse: Some(RunError::NoRoom),
        ..halves()
    };
    let (mut client, _serving) = served(opener);
    let refused = client
        .open(&job(folder.path(), b"net", None))
        .expect_err("no room");
    assert_eq!(refused, HelperError::Run(RunError::NoRoom));
}

#[test]
fn an_answer_that_is_not_finite_comes_back_with_its_sentence() {
    let folder = tempfile::tempdir().expect("a folder");
    let opener = StandIn {
        answers: Answers::NotFinite,
        ..halves()
    };
    let (mut client, _serving) = served(opener);
    client
        .open(&job(folder.path(), b"net", None))
        .expect("opened");
    let refused = client.block(&[0.9, 0.1]).expect_err("not finite");
    assert_eq!(refused, HelperError::Answer(FinishError::NotFinite));
}

#[test]
fn a_helper_that_ends_mid_block_is_a_failure_not_a_hang() {
    let folder = tempfile::tempdir().expect("a folder");
    let (mut from_volsa, to_helper) = std::io::pipe().expect("a pipe");
    let (from_helper, mut to_volsa) = std::io::pipe().expect("a pipe");
    let pretending = thread::spawn(move || {
        let _ = read_frame(&mut from_volsa);
        let _ = write_frame(&mut to_volsa, super::frame::OPENED, b"");
        let _ = read_frame(&mut from_volsa);
        // Ends here, without an answer: both pipes close.
    });
    let mut client = Client::new(from_helper, to_helper);
    client
        .open(&job(folder.path(), b"net", None))
        .expect("opened");
    let started = Instant::now();
    let refused = client.block(&[0.9, 0.1]).expect_err("ended");
    assert_eq!(refused, HelperError::Ended);
    assert_eq!(
        refused.to_string(),
        "The plugin stopped without finishing. Volsa is unaffected."
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    pretending.join().expect("ended");
}

#[test]
fn every_sentence_is_one_to_show() {
    let all = [
        HelperError::Ended,
        HelperError::Changed,
        HelperError::Run(RunError::NoRoom),
        HelperError::Answer(FinishError::Unnamed),
    ];
    for error in all {
        assert!(error.to_string().ends_with('.'));
    }
}
