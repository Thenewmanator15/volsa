//! Writing a label file: what it says in its header, and that what is written reads back as
//! it was, for a grid stored in each voxel order and for numbers of either width.

use std::io::Read;

use flate2::read::MultiGzDecoder;
use proptest::collection::vec;
use proptest::prelude::*;
use voxa_core::{Affine, Patient, Voxel};

use super::{WriteError, write_labels};
use crate::nifti::{LabelGrid, read_labels};

/// Numbers a label file holds: nothing, structures, and areas of the cortex.
const LABELS: [u32; 6] = [0, 2, 3, 41, 1001, 2035];

type Placement = Affine<Voxel, Patient>;

/// Where the voxels lie in each of four voxel orders: as scanned; every axis turned round;
/// columns running up and slices running to the left; and a plane turned a little.
fn placements() -> [(&'static str, Placement); 4] {
    [
        (
            "as scanned",
            Affine::from_columns(
                [1.25, 0.0, 0.0],
                [0.0, 1.25, 0.0],
                [0.0, 0.0, 3.5],
                [-8.5, 9.25, -11.5],
            ),
        ),
        (
            "turned round",
            Affine::from_columns(
                [-0.75, 0.0, 0.0],
                [0.0, -0.75, 0.0],
                [0.0, 0.0, -3.5],
                [11.5, -6.25, 23.5],
            ),
        ),
        (
            "columns up",
            Affine::from_columns(
                [0.0, 0.0, 1.75],
                [0.0, 1.0, 0.0],
                [6.0, 0.0, 0.0],
                [-14.5, 12.5, -15.5],
            ),
        ),
        (
            "turned a little",
            Affine::from_columns(
                [1.25, 0.75, 0.0],
                [-0.75, 1.25, 0.0],
                [0.0, 0.0, 3.5],
                [6.0, -17.5, 8.0],
            ),
        ),
    ]
}

/// A grid 8 by 6 by 4 whose numbers differ from place to place.
fn grid(placement: Placement) -> LabelGrid {
    let ids = (0..8 * 6 * 4_usize).map(|at| LABELS[(at % 7 + at / 5) % LABELS.len()]);
    LabelGrid {
        dims: [8, 6, 4],
        ids: ids.collect(),
        placement,
    }
}

/// A grid of one number at each place, in a 1 by 1 by n row.
fn row(ids: Vec<u32>) -> LabelGrid {
    LabelGrid {
        dims: [u32::try_from(ids.len()).expect("a few"), 1, 1],
        ids,
        placement: placements()[0].1,
    }
}

/// The bytes of a written file once unpacked.
fn unpacked(file: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    MultiGzDecoder::new(file)
        .read_to_end(&mut bytes)
        .expect("a gzip file");
    bytes
}

fn short_at(bytes: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn real_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// Whether two placements are the same to within 0.0001 in every part.
fn same_place(a: &Placement, b: &Placement) -> bool {
    let near = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1.0e-4);
    let (a_axes, b_axes) = (a.columns(), b.columns());
    near(a.origin(), b.origin()) && a_axes.iter().zip(b_axes).all(|(a, b)| near(*a, b))
}

fn assert_same(read: &LabelGrid, wanted: &LabelGrid, what: &str) {
    assert_eq!(read.dims, wanted.dims, "{what}");
    assert_eq!(read.ids, wanted.ids, "{what}");
    assert!(
        same_place(&read.placement, &wanted.placement),
        "{what}: {:?} against {:?}",
        read.placement,
        wanted.placement
    );
}

#[test]
fn a_grid_stored_in_any_voxel_order_is_read_back_as_it_was_placed() {
    for (what, placement) in placements() {
        let wanted = grid(placement);
        let file = write_labels(&wanted).expect("writes");
        assert_same(&read_labels(&file).expect("reads"), &wanted, what);
    }
}

#[test]
fn a_placement_a_float_cannot_hold_exactly_comes_back_within_a_ten_thousandth() {
    let placement = Affine::from_columns(
        [0.9, 0.0, 0.0],
        [0.0, -0.9, 0.0],
        [0.0, 0.0, 2.6],
        [123.456_789, -98.765_432, 10.1],
    );
    let wanted = grid(placement);
    let file = write_labels(&wanted).expect("writes");
    assert_same(&read_labels(&file).expect("reads"), &wanted, "rounded");
}

#[test]
fn the_header_says_what_other_programs_need_to_read_the_file() {
    let placement = Affine::from_columns(
        [1.25, 0.0, 0.0],
        [0.0, 1.25, 0.0],
        [0.0, 0.0, 3.5],
        [8.5, -9.25, 11.5],
    );
    let wanted = grid(placement);
    let file = write_labels(&wanted).expect("writes");
    assert_eq!(file[..2], [0x1f, 0x8b], "gzipped");
    let bytes = unpacked(&file);
    assert_eq!(
        bytes.len(),
        352 + 2 * wanted.ids.len(),
        "header, then 16 bits each"
    );
    assert_eq!(bytes[..4], 348_i32.to_le_bytes());
    assert_eq!(bytes[344..348], *b"n+1\0");
    assert_eq!(
        bytes[348..352],
        [0; 4],
        "the four bytes after the header say: no extensions"
    );
    assert_eq!(
        [40, 42, 44, 46, 48].map(|at| short_at(&bytes, at)),
        [3, 8, 6, 4, 1]
    );
    assert_eq!((short_at(&bytes, 70), short_at(&bytes, 72)), (512, 16));
    assert_eq!(
        real_at(&bytes, 108),
        352.0,
        "the values begin after the header"
    );
    assert_eq!((real_at(&bytes, 112), real_at(&bytes, 116)), (1.0, 0.0));
    assert_eq!(
        [76, 80, 84, 88].map(|at| real_at(&bytes, at)),
        [1.0, 1.25, 1.25, 3.5],
        "the size of a voxel each way"
    );
    assert_eq!(bytes[123], 2, "millimetres");
    assert_eq!(short_at(&bytes, 252), 0, "no qform");
    assert_eq!(short_at(&bytes, 254), 1, "an sform in the scanner's space");
    // The file's x and y run the other way from the patient's: the exact opposite of reading.
    let rows: Vec<f32> = (0..12)
        .map(|part| real_at(&bytes, 280 + 4 * part))
        .collect();
    assert_eq!(
        rows,
        [
            -1.25, 0.0, 0.0, -8.5, 0.0, -1.25, 0.0, 9.25, 0.0, 0.0, 3.5, 11.5
        ]
    );
    let first = u16::from_le_bytes([bytes[352], bytes[353]]);
    let second = u16::from_le_bytes([bytes[354], bytes[355]]);
    assert_eq!(
        (u32::from(first), u32::from(second)),
        (wanted.ids[0], wanted.ids[1]),
        "columns run fastest"
    );
}

#[test]
fn numbers_up_to_65535_are_stored_in_16_bits_and_a_larger_one_widens_all_to_32() {
    let narrow = row(vec![0, 7, 65_535]);
    let file = write_labels(&narrow).expect("writes");
    let bytes = unpacked(&file);
    assert_eq!((short_at(&bytes, 70), short_at(&bytes, 72)), (512, 16));
    assert_eq!(read_labels(&file).expect("reads").ids, narrow.ids);
    for biggest in [65_536, 70_000, i32::MAX.unsigned_abs()] {
        let wide = row(vec![0, 7, biggest]);
        let file = write_labels(&wide).expect("writes");
        let bytes = unpacked(&file);
        assert_eq!((short_at(&bytes, 70), short_at(&bytes, 72)), (8, 32));
        assert_eq!(bytes.len(), 352 + 4 * 3);
        assert_eq!(
            read_labels(&file).expect("reads").ids,
            wide.ids,
            "{biggest}"
        );
    }
    let too_big = row(vec![0, 2_147_483_648]);
    assert_eq!(write_labels(&too_big), Err(WriteError::TooLarge));
}

#[test]
fn a_grid_the_reader_would_refuse_for_its_size_is_not_written() {
    let sized = |dims: [u32; 3]| LabelGrid {
        dims,
        ids: Vec::new(),
        placement: placements()[0].1,
    };
    // A side that does not fit the sixteen bits a header gives it.
    let long = row(vec![1; 32_768]);
    assert_eq!(write_labels(&long), Err(WriteError::TooLarge));
    assert_eq!(
        write_labels(&sized([32_768, 1, 1])),
        Err(WriteError::TooLarge)
    );
    assert_eq!(
        write_labels(&sized([u32::MAX; 3])),
        Err(WriteError::TooLarge)
    );
    // Past the most voxels the reader holds, 2^28: one short of it is not "too large".
    assert_eq!(
        write_labels(&sized([16_384, 16_384, 2])),
        Err(WriteError::TooLarge)
    );
    assert_eq!(
        write_labels(&sized([16_384, 16_384, 1])),
        Err(WriteError::BadGrid)
    );
    let longest = row(vec![1; 32_767]);
    assert!(read_labels(&write_labels(&longest).expect("writes")).is_ok());
}

#[test]
fn a_grid_that_is_not_a_whole_volume_with_a_place_is_not_written() {
    let mut short = grid(placements()[0].1);
    short.ids.pop();
    assert_eq!(write_labels(&short), Err(WriteError::BadGrid));
    let mut long = grid(placements()[0].1);
    long.ids.push(2);
    assert_eq!(write_labels(&long), Err(WriteError::BadGrid));
    let mut flat = grid(placements()[0].1);
    flat.dims = [8, 6, 0];
    flat.ids.clear();
    assert_eq!(write_labels(&flat), Err(WriteError::BadGrid));
    let squashed = Affine::from_columns([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0; 3], [0.0; 3]);
    assert_eq!(write_labels(&grid(squashed)), Err(WriteError::BadGrid));
    let nowhere = Affine::from_columns(
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [f64::NAN; 3],
    );
    assert_eq!(write_labels(&grid(nowhere)), Err(WriteError::BadGrid));
    let too_far = Affine::from_columns(
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0e300; 3],
    );
    assert_eq!(
        write_labels(&grid(too_far)),
        Err(WriteError::BadGrid),
        "a place no 32-bit number can hold"
    );
}

#[test]
fn every_refusal_to_write_is_a_sentence_a_person_can_read() {
    for refusal in [
        WriteError::TooLarge,
        WriteError::NameTwice,
        WriteError::NoNames,
        WriteError::BadGrid,
    ] {
        let said = refusal.to_string();
        assert!(said.ends_with('.') && said.len() > 20, "{said}");
        for jargon in ["Error", "u16", "i32", "NIfTI", "sform", "::"] {
            assert!(!said.contains(jargon), "{said}");
        }
    }
}

proptest! {
    #[test]
    fn any_small_grid_comes_back_as_it_went(
        dims in [1_u32..6, 1_u32..6, 1_u32..6],
        ids in vec(0_u32..80_000, 125),
        steps in [0.5_f64..9.0, 0.5_f64..9.0, 0.5_f64..9.0],
        turned in [any::<bool>(); 3],
        start in [-30.0_f64..30.0, -30.0_f64..30.0, -30.0_f64..30.0],
    ) {
        let [x, y, z] = [0, 1, 2].map(|axis| {
            let step = steps[axis];
            if turned[axis] { -step } else { step }
        });
        let wanted = LabelGrid {
            dims,
            ids: ids.into_iter().take((dims[0] * dims[1] * dims[2]) as usize).collect(),
            placement: Affine::from_columns([x, 0.0, 0.0], [0.0, y, 0.0], [0.0, 0.0, z], start),
        };
        let file = write_labels(&wanted).expect("writes");
        let read = read_labels(&file).expect("reads");
        prop_assert_eq!(&read.ids, &wanted.ids);
        prop_assert_eq!(read.dims, wanted.dims);
        prop_assert!(same_place(&read.placement, &wanted.placement));
    }
}
