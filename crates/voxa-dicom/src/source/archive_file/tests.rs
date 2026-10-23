use std::io::{Cursor, Read, Seek, SeekFrom, Write};

use proptest::prelude::*;

use super::*;

/// A file holding `bytes`, opened as an archive file.
fn holding(bytes: &[u8]) -> ArchiveFile {
    let mut file = tempfile::tempfile().expect("a temporary file");
    file.write_all(bytes).expect("written");
    ArchiveFile::new(file)
}

fn counting(len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| u8::try_from(i % 251).expect("below 251"))
        .collect()
}

/// One step of a test: move somewhere, then read up to some bytes.
#[derive(Debug, Clone)]
enum Step {
    Start(u64),
    End(i64),
    Current(i64),
    Read(usize),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0u64..30_000).prop_map(Step::Start),
        (-30_000i64..100).prop_map(Step::End),
        (-10_000i64..10_000).prop_map(Step::Current),
        (0usize..20_000).prop_map(Step::Read),
    ]
}

/// What a reader gives for one step: where it now is or what it read, or the kind of error.
fn outcome<R: Read + Seek>(
    reader: &mut R,
    step: &Step,
) -> Result<(u64, Vec<u8>), std::io::ErrorKind> {
    let to = match step {
        Step::Start(at) => SeekFrom::Start(*at),
        Step::End(by) => SeekFrom::End(*by),
        Step::Current(by) => SeekFrom::Current(*by),
        Step::Read(wanted) => {
            let mut out = vec![0; *wanted];
            let count = reader.read(&mut out).map_err(|e| e.kind())?;
            out.truncate(count);
            return Ok((0, out));
        }
    };
    reader
        .seek(to)
        .map(|at| (at, Vec::new()))
        .map_err(|e| e.kind())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn it_reads_and_moves_as_a_cursor_over_the_same_bytes_does(
        len in 0usize..25_000,
        steps in proptest::collection::vec(step(), 1..40),
    ) {
        let bytes = counting(len);
        let mut file = holding(&bytes);
        let mut cursor = Cursor::new(bytes);
        for step in &steps {
            let expected = outcome(&mut cursor, step);
            prop_assert_eq!(outcome(&mut file, step), expected, "at {:?}", step);
        }
    }
}

#[test]
fn a_clone_keeps_a_position_of_its_own() {
    let bytes = counting(50_000);
    let mut first = holding(&bytes);
    let mut start = [0u8; 100];
    first.read_exact(&mut start).expect("reads");
    let mut second = first.clone();
    second.seek(SeekFrom::Start(40_000)).expect("moves");
    let (mut from_first, mut from_second) = ([0u8; 10], [0u8; 10]);
    first.read_exact(&mut from_first).expect("reads");
    second.read_exact(&mut from_second).expect("reads");
    assert_eq!(from_first.as_slice(), &bytes[100..110]);
    assert_eq!(from_second.as_slice(), &bytes[40_000..40_010]);
    let mut again = [0u8; 10];
    first.read_exact(&mut again).expect("reads");
    assert_eq!(
        again.as_slice(),
        &bytes[110..120],
        "the clone did not move the first"
    );
}

#[test]
fn clones_read_on_several_threads_at_once_each_get_their_own_bytes() {
    let bytes = counting(400_000);
    let file = holding(&bytes);
    std::thread::scope(|scope| {
        for thread in 0..8u64 {
            let mut reader = file.clone();
            let bytes = &bytes;
            scope.spawn(move || {
                for round in 0..200u64 {
                    let at = (thread * 37_000 + round * 1_013) % 390_000;
                    let wanted = if round % 2 == 0 { 100 } else { 9_000 };
                    let mut out = vec![0u8; wanted];
                    reader.seek(SeekFrom::Start(at)).expect("moves");
                    reader.read_exact(&mut out).expect("reads");
                    let at = usize::try_from(at).expect("small");
                    assert_eq!(out.as_slice(), &bytes[at..at + wanted]);
                }
            });
        }
    });
}
