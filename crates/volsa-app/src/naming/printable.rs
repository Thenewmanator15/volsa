//! Text from a file made fit to draw on one line: no character that moves, hides or joins
//! the letters around it.
//!
//! A file's text may hold control characters, such as a line break, and format characters
//! (Unicode's category Cf), such as the marks that turn text right to left or a space of no
//! width. Drawn, they could break a line in two, or make one word look like another. Every
//! letter a person can see is kept as it is.

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
pub fn is_format(letter: char) -> bool {
    let code = u32::from(letter);
    FORMAT
        .iter()
        .any(|(first, last)| (*first..=*last).contains(&code))
}

/// The text with each control character taken for a space and each format character left
/// out.
pub fn printable(text: &str) -> String {
    text.chars()
        .filter(|letter| !is_format(*letter))
        .map(|letter| if letter.is_control() { ' ' } else { letter })
        .collect()
}

/// Like [`printable`], for text drawn over several lines: each end of a line (`\n`, `\r\n` or
/// `\r`) stays as one `\n`, so that paragraphs and lists keep their lines, and every other
/// control character is taken for a space and every format character left out.
pub fn printable_multiline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut letters = text.chars().peekable();
    while let Some(letter) = letters.next() {
        match letter {
            '\r' => {
                letters.next_if_eq(&'\n');
                out.push('\n');
            }
            _ if is_format(letter) => {}
            _ if letter.is_control() && letter != '\n' => out.push(' '),
            _ => out.push(letter),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_of_several_paragraphs_keeps_its_line_breaks_and_nothing_else_that_breaks() {
        // Every way of ending a line is one line break; a tab is a space; marks that turn or hide
        // letters are left out, wherever they are.
        assert_eq!(
            printable_multiline("one\ttwo\r\nthree\rfour\nfive"),
            "one two\nthree\nfour\nfive"
        );
        assert_eq!(
            printable_multiline("No\u{200B}te\n\u{202E}x\u{1b}y"),
            "Note\nx y"
        );
        assert_eq!(printable_multiline("\n\n"), "\n\n", "blank lines stay");
        assert_eq!(printable_multiline("Tête\n頭部"), "Tête\n頭部");
        // The single line is still one line.
        assert_eq!(printable("one\r\ntwo"), "one  two");
    }

    #[test]
    fn what_turns_hides_or_breaks_text_is_left_out_and_every_letter_is_kept() {
        // A mark that turns the text right to left would show "T1" as "1T".
        assert_eq!(printable("ax \u{202E}T1\u{202C} flair"), "ax T1 flair");
        assert_eq!(printable("T2\u{200B}\u{2066}w\u{2069}"), "T2w");
        assert_eq!(printable("head\nneck\tspine"), "head neck spine");
        assert_eq!(printable("\u{FEFF}Kopf, axial"), "Kopf, axial");
        assert_eq!(printable("Tête — coupe 3D"), "Tête — coupe 3D");
        assert_eq!(printable("頭部 T1"), "頭部 T1");
        for letter in ['\u{AD}', '\u{200E}', '\u{2061}', '\u{E0041}'] {
            assert!(is_format(letter), "{:X}", u32::from(letter));
        }
        for letter in ['a', ' ', '-', 'é', '\u{2010}', '\u{2070}'] {
            assert!(!is_format(letter), "{:X}", u32::from(letter));
        }
    }
}
