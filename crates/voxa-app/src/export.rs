//! Turning pictures of the 3D scene into files another program can open: a PNG for one
//! picture, an animated GIF for a turntable. Each holds the picture and nothing else: no
//! name, date, description or identifier from the scan.

use std::io::Cursor;
use std::path::Path;

use image::codecs::gif::{GifEncoder, Repeat};
use image::codecs::png::PngEncoder;
use image::{Delay, ExtendedColorType, Frame, ImageEncoder, RgbaImage};

/// The file name offered when a picture or a turntable is saved. It says nothing of the scan.
pub const SUGGESTED_NAME: &str = "voxa-3d";
/// How many pictures a turntable is made of: one every five degrees.
pub const TURNTABLE_FRAMES: u32 = 72;
/// How long a turntable takes to go round once, in seconds.
pub const TURNTABLE_SECONDS: f32 = 6.0;
/// The longer side of a turntable's pictures, at the most.
const TURNTABLE_SIDE: f32 = 512.0;

/// Why a picture or a turntable could not be saved. Each reads as a sentence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExportError {
    /// There is no picture, or its bytes are not four for every pixel of its size.
    #[error("There is no picture of that size to save.")]
    Size,
    /// The picture could not be put into the file's format.
    #[error("The picture could not be made into a file: {0}.")]
    Encode(String),
    /// The file could not be written.
    #[error("The file could not be written: {0}.")]
    Write(String),
}

/// Whether `rgba` holds four bytes for every pixel of a picture of `size`, which has a size.
fn fits(rgba: &[u8], [width, height]: [u32; 2]) -> bool {
    let bytes = u64::from(width) * u64::from(height) * 4;
    bytes > 0 && rgba.len() as u64 == bytes
}

/// One picture as a PNG file. `rgba` is red, green, blue, alpha bytes, row by row from the
/// top, and `size` its width and height.
///
/// # Errors
///
/// [`ExportError::Size`] when the bytes are not those of a picture of that size, and
/// [`ExportError::Encode`] when the encoder fails.
pub fn png(rgba: &[u8], size: [u32; 2]) -> Result<Vec<u8>, ExportError> {
    if !fits(rgba, size) {
        return Err(ExportError::Size);
    }
    let [width, height] = size;
    let mut file = Vec::new();
    PngEncoder::new(&mut file)
        .write_image(rgba, width, height, ExtendedColorType::Rgba8)
        .map_err(|why| ExportError::Encode(why.to_string()))?;
    Ok(file)
}

/// Pictures of one size as an animated GIF that plays them in order, over and over, taking
/// `seconds` to go through them all. A grey picture loses nothing: a GIF holds 256 levels.
///
/// # Errors
///
/// [`ExportError::Size`] when there are no pictures or one is not of that size, and
/// [`ExportError::Encode`] when the encoder fails.
pub fn gif(frames: &[Vec<u8>], size: [u32; 2], seconds: f32) -> Result<Vec<u8>, ExportError> {
    if frames.is_empty() || !frames.iter().all(|frame| fits(frame, size)) {
        return Err(ExportError::Size);
    }
    let [width, height] = size;
    let each_ms = seconds.max(0.0) * 1000.0 / frames.len() as f32;
    // A GIF counts time in hundredths of a second, and players treat none at all as a lot.
    let delay = Delay::from_numer_denom_ms(whole(each_ms).max(20), 1);
    let failed = |why: image::ImageError| ExportError::Encode(why.to_string());
    let mut file = Cursor::new(Vec::new());
    {
        let mut encoder = GifEncoder::new(&mut file);
        encoder.set_repeat(Repeat::Infinite).map_err(failed)?;
        for rgba in frames {
            let picture =
                RgbaImage::from_raw(width, height, rgba.clone()).ok_or(ExportError::Size)?;
            encoder
                .encode_frame(Frame::from_parts(picture, 0, 0, delay))
                .map_err(failed)?;
        }
    }
    Ok(file.into_inner())
}

/// The size, in pixels, of a turntable's pictures for a scene laid out at `viewport`: the
/// same shape, its longer side 512 at the most, each side at least 1.
pub fn turntable_size(viewport: [f32; 2]) -> [u32; 2] {
    let longer = viewport.into_iter().fold(1.0_f32, f32::max);
    let shrink = (TURNTABLE_SIDE / longer).min(1.0);
    viewport.map(|side| whole(side * shrink).max(1))
}

/// Write a file's bytes where the person chose.
///
/// # Errors
///
/// [`ExportError::Write`] with the system's reason.
pub fn write(path: &Path, file: &[u8]) -> Result<(), ExportError> {
    std::fs::write(path, file).map_err(|why| ExportError::Write(why.to_string()))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held in range first"
)]
fn whole(length: f32) -> u32 {
    if length.is_finite() {
        length.round().clamp(0.0, 1.0e9) as u32
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use image::AnimationDecoder;
    use image::codecs::gif::GifDecoder;

    use super::*;

    /// A picture of greys that change from pixel to pixel and from frame to frame.
    fn greys(size: [u32; 2], frame: u8) -> Vec<u8> {
        let pixels = u8::try_from(size[0] * size[1]).expect("a small picture");
        let grey = |pixel: u8| pixel.wrapping_mul(3).wrapping_add(frame.wrapping_mul(40));
        (0..pixels)
            .flat_map(|pixel| [grey(pixel), grey(pixel), grey(pixel), 255])
            .collect()
    }

    #[test]
    fn a_picture_saved_as_png_is_read_back_the_same() {
        let rgba: Vec<u8> = (0..24u8).map(|byte| byte.wrapping_mul(11)).collect();
        let file = png(&rgba, [3, 2]).expect("encodes");
        let read = image::load_from_memory(&file).expect("is a png").to_rgba8();
        assert_eq!((read.width(), read.height()), (3, 2));
        assert_eq!(read.into_raw(), rgba);
    }

    #[test]
    fn bytes_that_are_not_a_picture_of_the_size_given_are_refused() {
        assert_eq!(png(&[0; 23], [3, 2]), Err(ExportError::Size));
        assert_eq!(png(&[], [0, 0]), Err(ExportError::Size));
        assert_eq!(gif(&[], [8, 8], 6.0), Err(ExportError::Size), "no frames");
        assert_eq!(gif(&[vec![0; 12]], [8, 8], 6.0), Err(ExportError::Size));
    }

    #[test]
    fn frames_saved_as_gif_are_read_back_in_order_with_their_greys() {
        let size = [8, 8];
        let frames: Vec<Vec<u8>> = (0..4).map(|frame| greys(size, frame)).collect();
        let file = gif(&frames, size, 2.0).expect("encodes");
        let decoder = GifDecoder::new(Cursor::new(file)).expect("is a gif");
        let read: Vec<Frame> = decoder.into_frames().collect_frames().expect("decodes");
        assert_eq!(read.len(), 4);
        for (read, written) in read.iter().zip(&frames) {
            assert_eq!(read.buffer().dimensions(), (8, 8));
            let apart = read
                .buffer()
                .as_raw()
                .iter()
                .zip(written)
                .map(|(a, b)| a.abs_diff(*b));
            assert!(apart.max() <= Some(1), "a grey is out by more than a level");
            let (ms, per) = read.delay().numer_denom_ms();
            assert_eq!(ms / per, 500, "four frames in two seconds");
        }
    }

    #[test]
    fn a_turntable_is_no_more_than_512_pixels_on_its_longer_side() {
        assert_eq!(turntable_size([1024.0, 768.0]), [512, 384]);
        assert_eq!(turntable_size([300.0, 600.0]), [256, 512]);
        assert_eq!(
            turntable_size([400.0, 200.0]),
            [400, 200],
            "a small one is not enlarged"
        );
        assert_eq!(turntable_size([0.0, f32::NAN]), [1, 1]);
    }

    #[test]
    fn a_file_is_written_where_asked_or_the_reason_is_given() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let path = folder.path().join(format!("{SUGGESTED_NAME}.png"));
        write(&path, b"picture").expect("writes");
        assert_eq!(std::fs::read(&path).expect("reads"), b"picture");
        let nowhere = folder.path().join("no such folder").join("x.png");
        assert!(matches!(
            write(&nowhere, b"picture"),
            Err(ExportError::Write(_))
        ));
        assert_eq!(SUGGESTED_NAME, "voxa-3d");
    }

    /// The kinds of chunk a PNG file holds, in order.
    fn png_chunks(file: &[u8]) -> Vec<String> {
        let mut kinds = Vec::new();
        let mut at = 8;
        while let Some(head) = file.get(at..at + 8) {
            let (length, kind) = head.split_at(4);
            let length = u32::from_be_bytes(length.try_into().expect("four bytes"));
            kinds.push(String::from_utf8_lossy(kind).into_owned());
            // The length, the kind, the data and the check value.
            at += 12 + usize::try_from(length).expect("a small chunk");
        }
        kinds
    }

    /// Skip the sub-blocks that begin at `at` in a GIF file, to the byte after the last.
    fn after_sub_blocks(file: &[u8], mut at: usize) -> usize {
        loop {
            let size = usize::from(file.get(at).copied().expect("a block ends"));
            at += 1 + size;
            if size == 0 {
                return at;
            }
        }
    }

    /// The label of each extension block a GIF file holds, in order.
    fn gif_extensions(file: &[u8]) -> Vec<u8> {
        let table = |flags: u8| {
            if flags & 0x80 == 0 {
                0
            } else {
                3 << ((flags & 7) + 1)
            }
        };
        // After the signature and the screen description, and its colour table if it has one.
        let screen = file.get(10).copied().expect("a screen description");
        let mut at = 13 + table(screen);
        let mut labels = Vec::new();
        loop {
            match file.get(at).copied().expect("the file ends with a trailer") {
                0x21 => {
                    labels.push(file.get(at + 1).copied().expect("a label"));
                    at = after_sub_blocks(file, at + 2);
                }
                0x2C => {
                    let flags = file.get(at + 9).copied().expect("a picture description");
                    // Ten bytes, the picture's own colour table, then its code size.
                    at = after_sub_blocks(file, at + 10 + table(flags) + 1);
                }
                _ => return labels,
            }
        }
    }

    #[test]
    fn a_saved_picture_and_a_saved_turntable_hold_nothing_but_pixels() {
        let size = [8, 8];
        let frames: Vec<Vec<u8>> = (0..3).map(|frame| greys(size, frame)).collect();
        // A PNG holds its header, its pixels and its end: no text, no time, no camera.
        let file = png(&frames[0], size).expect("encodes");
        assert_eq!(&file[..8], b"\x89PNG\r\n\x1a\n");
        let chunks = png_chunks(&file);
        for kind in ["tEXt", "zTXt", "iTXt", "eXIf", "tIME"] {
            assert!(!chunks.iter().any(|one| one == kind), "{chunks:?}");
        }
        assert_eq!(chunks.first().map(String::as_str), Some("IHDR"));
        assert_eq!(chunks.last().map(String::as_str), Some("IEND"));
        // A GIF holds no comment or text, only what makes it play over and over.
        let file = gif(&frames, size, 2.0).expect("encodes");
        assert_eq!(&file[..3], b"GIF");
        let labels = gif_extensions(&file);
        assert!(!labels.contains(&0xFE), "a comment: {labels:?}");
        assert!(!labels.contains(&0x01), "a text: {labels:?}");
        assert!(labels.contains(&0xF9), "its frames have delays: {labels:?}");
        // The look-outs do see what they look for: a text put in a PNG after its header, and a
        // comment put in a GIF before its trailer, are found.
        let plain = png(&frames[0], size).expect("encodes");
        let (head, rest) = plain.split_at(33);
        let text = [&[0, 0, 0, 3][..], b"tEXtabc", &[0; 4]].concat();
        let dirty_png = [head, &text[..], rest].concat();
        assert!(png_chunks(&dirty_png).contains(&"tEXt".to_owned()));
        let (head, trailer) = file.split_at(file.len() - 1);
        let comment = [0x21, 0xFE, 3, b'a', b'b', b'c', 0];
        let dirty_gif = [head, &comment[..], trailer].concat();
        assert!(gif_extensions(&dirty_gif).contains(&0xFE));
    }

    #[test]
    fn each_reason_reads_as_a_sentence() {
        assert_eq!(
            ExportError::Size.to_string(),
            "There is no picture of that size to save."
        );
        let disk = ExportError::Write("the disk is full".to_owned()).to_string();
        assert_eq!(disk, "The file could not be written: the disk is full.");
    }
}
