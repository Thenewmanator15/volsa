//! A frame is a kind, a length and that many bytes: read back as written, and refused, never
//! a panic, when it is too long or cut short.

use std::io::Cursor;

use super::frame::{FrameError, read_frame, write_frame};

#[test]
fn a_frame_round_trips() {
    let mut written = Vec::new();
    write_frame(&mut written, 7, b"seven").expect("written");
    write_frame(&mut written, 9, b"").expect("written");
    let mut reading = Cursor::new(written);
    assert_eq!(
        read_frame(&mut reading).expect("read"),
        Some((7, b"seven".to_vec()))
    );
    assert_eq!(
        read_frame(&mut reading).expect("read"),
        Some((9, Vec::new()))
    );
    assert_eq!(read_frame(&mut reading).expect("read"), None);
}

#[test]
fn a_length_over_one_gibibyte_is_refused_before_anything_is_read() {
    let mut header = vec![2_u8];
    header.extend_from_slice(&((1_u64 << 30) + 1).to_le_bytes());
    let refused = read_frame(&mut Cursor::new(header)).expect_err("too long");
    assert_eq!(refused, FrameError::TooLong);
}

#[test]
fn a_frame_cut_short_is_an_error_not_a_panic() {
    let mut written = Vec::new();
    write_frame(&mut written, 3, b"payload").expect("written");
    for cut in 1..written.len() {
        let refused = read_frame(&mut Cursor::new(&written[..cut])).expect_err("cut short");
        assert_eq!(refused, FrameError::Cut, "cut at {cut}");
    }
}
