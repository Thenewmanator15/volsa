//! Image files whose sequences nest one inside another as deep as a test asks: the shape of a
//! damaged or crafted file that would have the DICOM library recurse until its thread runs out
//! of stack.
//!
//! The nesting is written byte by byte. The library's own writer recurses as its reader does,
//! so it could not write a file deep enough to test with.

use std::io::Cursor;

use dicom_core::Tag;
use dicom_dictionary_std::tags;
use dicom_object::FileMetaTableBuilder;

use super::build::IMPLEMENTATION_CLASS;
use super::{EXPLICIT_VR_LITTLE_ENDIAN, SliceOptions, classic_slice};

/// Transfer syntax of implicit VR little endian.
pub const IMPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2";

/// A made-up image with sequences nested in it. Start from [`Nesting::of`].
#[derive(Debug, Clone)]
pub struct Nesting {
    /// The image the nesting is put in.
    pub slice: SliceOptions,
    /// How many sequences deep the nesting goes: 1 is one sequence whose item holds no other.
    pub depth: usize,
    /// Write the data set as implicit VR little endian rather than explicit.
    pub implicit: bool,
    /// Give every sequence and item its length, rather than closing each with a delimiter.
    pub defined_lengths: bool,
    /// Put the nesting after the pixel data, where only a read of the whole file meets it.
    pub after_pixels: bool,
    /// Put a private element of this many bytes, an even number, just before the nesting.
    pub padding: usize,
}

impl Nesting {
    /// Nesting `depth` sequences deep in the default slice, before its pixels: explicit VR,
    /// each sequence and item closed by a delimiter.
    pub fn of(depth: usize) -> Self {
        Self {
            slice: SliceOptions::default(),
            depth,
            implicit: false,
            defined_lengths: false,
            after_pixels: false,
            padding: 0,
        }
    }
}

/// One image file with sequences nested `nesting.depth` deep, otherwise as [`classic_slice`]
/// writes it. Before the pixels, the nesting is a content sequence as a structured report's
/// tree of items has; after them, it is a sequence of digital signatures.
pub fn nested_slice(nesting: &Nesting) -> Vec<u8> {
    let (mut bytes, pixels) = without_pixels(&nesting.slice, nesting.implicit);
    let tag = if nesting.after_pixels {
        tags::DIGITAL_SIGNATURES_SEQUENCE
    } else {
        tags::CONTENT_SEQUENCE
    };
    let mut nested = Vec::new();
    if nesting.padding > 0 {
        let padding = len(nesting.padding);
        put_header(&mut nested, PADDING, *b"OB", padding, nesting.implicit);
        nested.resize(nested.len() + nesting.padding, 0);
    }
    let sequences = nested_sequences(
        tag,
        nesting.depth,
        nesting.implicit,
        nesting.defined_lengths,
    );
    nested.extend(sequences);
    if !nesting.after_pixels {
        bytes.extend_from_slice(&nested);
    }
    let pixel_len = len(pixels.len());
    put_header(
        &mut bytes,
        tags::PIXEL_DATA,
        *b"OW",
        pixel_len,
        nesting.implicit,
    );
    bytes.extend_from_slice(&pixels);
    if nesting.after_pixels {
        bytes.extend_from_slice(&nested);
    }
    bytes
}

/// The file [`classic_slice`] writes for `slice`, its data set in implicit or explicit VR, up
/// to but not including its pixel data; and the bytes of its pixels.
fn without_pixels(slice: &SliceOptions, implicit: bool) -> (Vec<u8>, Vec<u8>) {
    let plain = classic_slice(slice);
    let mut file = dicom_object::from_reader(Cursor::new(plain)).expect("the made-up slice parses");
    let pixels = file
        .element(tags::PIXEL_DATA)
        .expect("the made-up slice has pixels")
        .to_bytes()
        .expect("its pixels are bytes")
        .into_owned();
    file.remove_element(tags::PIXEL_DATA);
    let syntax = if implicit {
        IMPLICIT_VR_LITTLE_ENDIAN
    } else {
        EXPLICIT_VR_LITTLE_ENDIAN
    };
    let file = file
        .into_inner()
        .with_meta(
            FileMetaTableBuilder::new()
                .transfer_syntax(syntax)
                .implementation_class_uid(IMPLEMENTATION_CLASS),
        )
        .expect("the made-up slice has its identifiers");
    let mut bytes = vec![0u8; 128];
    bytes.extend_from_slice(b"DICM");
    file.write_meta(&mut bytes)
        .expect("writing to memory does not fail");
    file.write_dataset(&mut bytes)
        .expect("writing to memory does not fail");
    (bytes, pixels)
}

/// The bytes of `depth` sequences of `tag`, each the one item of the sequence around it.
fn nested_sequences(tag: Tag, depth: usize, implicit: bool, defined: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(depth.saturating_mul(36));
    if defined {
        // The length of the item at each level, from the outside in; the innermost is empty.
        let header = if implicit { 8 } else { 12 };
        let mut items = vec![0u32; depth];
        let mut inner = 0u32;
        for item in items.iter_mut().rev() {
            *item = inner;
            inner = (header + ITEM_HEADER)
                .checked_add(inner)
                .expect("the nesting fits in a 32-bit length");
        }
        for item in items {
            put_header(&mut out, tag, *b"SQ", ITEM_HEADER + item, implicit);
            put_item(&mut out, ITEM, item);
        }
    } else {
        for _ in 0..depth {
            put_header(&mut out, tag, *b"SQ", UNDEFINED, implicit);
            put_item(&mut out, ITEM, UNDEFINED);
        }
        for _ in 0..depth {
            put_item(&mut out, ITEM_END, 0);
            put_item(&mut out, SEQUENCE_END, 0);
        }
    }
    out
}

/// A private element, of no meaning to any reader.
const PADDING: Tag = Tag(0x0009, 0x1010);
const UNDEFINED: u32 = u32::MAX;
const ITEM_HEADER: u32 = 8;
const ITEM: Tag = Tag(0xFFFE, 0xE000);
const ITEM_END: Tag = Tag(0xFFFE, 0xE00D);
const SEQUENCE_END: Tag = Tag(0xFFFE, 0xE0DD);

/// An element's header, little endian: tag and length, with the VR between them when explicit.
fn put_header(out: &mut Vec<u8>, tag: Tag, vr: [u8; 2], len: u32, implicit: bool) {
    put_tag(out, tag);
    if implicit {
        out.extend_from_slice(&len.to_le_bytes());
    } else {
        out.extend_from_slice(&vr);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&len.to_le_bytes());
    }
}

/// An item's header or delimiter: its tag and a 32-bit length, whatever the VR encoding.
fn put_item(out: &mut Vec<u8>, tag: Tag, len: u32) {
    put_tag(out, tag);
    out.extend_from_slice(&len.to_le_bytes());
}

fn put_tag(out: &mut Vec<u8>, tag: Tag) {
    out.extend_from_slice(&tag.group().to_le_bytes());
    out.extend_from_slice(&tag.element().to_le_bytes());
}

fn len(bytes: usize) -> u32 {
    u32::try_from(bytes).expect("the made-up pixels fit in a 32-bit length")
}
