//! The rule for every word a description holds: it is shown as it is, so it may hold no
//! character that is not drawn but changes how the letters around it are drawn.
//!
//! `FORMAT` and `is_format` are the redesign's own (`naming/printable.rs` on the interface
//! redesign's branch): one copy is kept when the two branches meet.

/// The format characters of Unicode 15.1 (category Cf), as ranges of code points.
const FORMAT: [(u32, u32); 21] = [
    (0x00AD, 0x00AD),
    (0x0600, 0x0605),
    (0x061C, 0x061C),
    (0x06DD, 0x06DD),
    (0x070F, 0x070F),
    (0x0890, 0x0891),
    (0x08E2, 0x08E2),
    (0x180E, 0x180E),
    (0x200B, 0x200F),
    (0x202A, 0x202E),
    (0x2060, 0x2064),
    (0x2066, 0x206F),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
];

/// Whether a character is a format character: one that is not drawn, but changes how the
/// letters around it are drawn.
fn is_format(letter: char) -> bool {
    let code = u32::from(letter);
    FORMAT
        .iter()
        .any(|(first, last)| (*first..=*last).contains(&code))
}

/// Whether every character of the text can be shown as it is: none a control character and
/// none a format character.
pub(super) fn showable(text: &str) -> bool {
    !text
        .chars()
        .any(|letter| letter.is_control() || is_format(letter))
}

/// Whether a text is a plugin's identity: 3 to 64 of `a-z`, `0-9`, dots and dashes.
pub(super) fn is_id(text: &str) -> bool {
    (3..=64).contains(&text.len())
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-'
        })
}

/// Whether a text is a SHA-256 sum: 64 hexadecimal digits.
pub(super) fn is_sum(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}
