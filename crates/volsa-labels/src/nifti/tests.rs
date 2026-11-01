//! Reading label volumes: the kinds of number, either byte order, gzip, where the voxels
//! lie, and every way a file can be wrong.

use volsa_core::Point;

use super::*;
use crate::testkit::{NiftiOptions, Quatern, gzipped, nifti};

/// 2 by 3 by 4 voxels numbered from 0 in the order stored, 1 mm apart, the first at
/// (10, 20, 30) in the file's own right-anterior-superior world.
fn counting() -> NiftiOptions {
    NiftiOptions {
        dims: [2, 3, 4],
        ids: (0..24).collect(),
        kind: 512,
        srow: Some([
            [1.0, 0.0, 0.0, 10.0],
            [0.0, 1.0, 0.0, 20.0],
            [0.0, 0.0, 1.0, 30.0],
        ]),
        quatern: None,
        big_endian: false,
        slope: 1.0,
    }
}

fn patient(grid: &LabelGrid, voxel: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = voxel;
    let at = grid.placement.apply(Point::new(x, y, z));
    [at.x, at.y, at.z]
}

fn close(a: [f64; 3], b: [f64; 3]) -> bool {
    a.into_iter().zip(b).all(|(a, b)| (a - b).abs() < 1.0e-5)
}

/// Write a 16-bit number into a header, in the little-endian order the test files use.
fn put(bytes: &mut [u8], at: usize, value: i16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn a_label_volume_is_read_with_its_numbers_in_order_and_placed_in_the_patient() {
    let grid = read_labels(&nifti(&counting())).expect("reads");
    assert_eq!(grid.dims, [2, 3, 4]);
    assert_eq!(grid.ids, (0..24).collect::<Vec<u32>>());
    // The file's world has x to the patient's right and y to the front; Volsa's has them to
    // the left and the back. Up is up in both.
    let at = patient(&grid, [1.0, 2.0, 3.0]);
    assert!(close(at, [-11.0, -22.0, 33.0]), "{at:?}");
    assert!(close(patient(&grid, [0.0; 3]), [-10.0, -20.0, 30.0]));
}

#[test]
fn gzip_either_byte_order_and_every_kind_of_whole_number_give_the_same_volume() {
    let plain = read_labels(&nifti(&counting())).expect("reads");
    let zipped = read_labels(&gzipped(&nifti(&counting()))).expect("reads gzip");
    assert_eq!(zipped, plain);
    let other_order = NiftiOptions {
        big_endian: true,
        ..counting()
    };
    assert_eq!(read_labels(&nifti(&other_order)).expect("reads"), plain);
    for kind in [2, 256, 4, 512, 8, 768, 16, 64] {
        for big_endian in [false, true] {
            let options = NiftiOptions {
                kind,
                big_endian,
                ..counting()
            };
            let read = read_labels(&nifti(&options));
            assert_eq!(
                read.as_ref(),
                Ok(&plain),
                "kind {kind}, big-endian {big_endian}"
            );
        }
    }
    let unscaled = NiftiOptions {
        slope: 0.0,
        ..counting()
    };
    assert_eq!(
        read_labels(&nifti(&unscaled)).expect("reads"),
        plain,
        "0 means not scaled"
    );
}

#[test]
fn a_file_placed_by_a_quaternion_is_placed_by_it() {
    // A quarter turn about the axis that points up, voxels 2 by 3 by 4 mm.
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let turned = |qfac| NiftiOptions {
        srow: None,
        quatern: Some(Quatern {
            turn: [0.0, 0.0, half],
            first: [10.0, 20.0, 30.0],
            sizes: [2.0, 3.0, 4.0],
            flip: qfac,
        }),
        ..counting()
    };
    let grid = read_labels(&nifti(&turned(1.0))).expect("reads");
    // One voxel along the columns is 2 mm towards the front of the file's world.
    let at = patient(&grid, [1.0, 0.0, 0.0]);
    assert!(close(at, [-10.0, -22.0, 30.0]), "{at:?}");
    let at = patient(&grid, [0.0, 1.0, 0.0]);
    assert!(
        close(at, [-7.0, -20.0, 30.0]),
        "3 mm towards its left: {at:?}"
    );
    assert!(close(patient(&grid, [0.0, 0.0, 1.0]), [-10.0, -20.0, 34.0]));
    let mirrored = read_labels(&nifti(&turned(-1.0))).expect("reads");
    let at = patient(&mirrored, [0.0, 0.0, 1.0]);
    assert!(
        close(at, [-10.0, -20.0, 26.0]),
        "the third axis runs the other way: {at:?}"
    );
}

#[test]
fn the_sform_is_used_when_a_file_has_both() {
    let both = NiftiOptions {
        quatern: Some(Quatern {
            turn: [0.0; 3],
            first: [500.0; 3],
            sizes: [9.0; 3],
            flip: 1.0,
        }),
        ..counting()
    };
    let grid = read_labels(&nifti(&both)).expect("reads");
    assert!(close(patient(&grid, [1.0, 2.0, 3.0]), [-11.0, -22.0, 33.0]));
}

#[test]
fn what_is_not_a_single_file_nifti_1_is_refused_in_so_many_words() {
    assert_eq!(read_labels(&[0; 10]), Err(NiftiError::NotNifti));
    assert_eq!(read_labels(&[]), Err(NiftiError::NotNifti));
    assert_eq!(read_labels(&[7; 400]), Err(NiftiError::NotNifti));
    let mut second_version = nifti(&counting());
    second_version[..4].copy_from_slice(&540_i32.to_le_bytes());
    assert_eq!(read_labels(&second_version), Err(NiftiError::Nifti2));
    let mut two_files = nifti(&counting());
    two_files[344..348].copy_from_slice(b"ni1\0");
    assert_eq!(read_labels(&two_files), Err(NiftiError::TwoFiles));
    let mut no_magic = nifti(&counting());
    no_magic[344..348].copy_from_slice(b"abcd");
    assert_eq!(read_labels(&no_magic), Err(NiftiError::NotNifti));
}

#[test]
fn a_file_that_is_not_one_volume_of_labels_is_refused() {
    let mut over_time = nifti(&counting());
    put(&mut over_time, 40, 4);
    put(&mut over_time, 48, 2);
    assert_eq!(read_labels(&over_time), Err(NiftiError::NotThreeD));
    let mut one_moment = nifti(&counting());
    put(&mut one_moment, 40, 4);
    put(&mut one_moment, 48, 1);
    assert!(
        read_labels(&one_moment).is_ok(),
        "a fourth dimension of 1 is no dimension"
    );
    let colour = NiftiOptions {
        kind: 128,
        ..counting()
    };
    assert_eq!(read_labels(&nifti(&colour)), Err(NiftiError::Kind(128)));
    let scaled = NiftiOptions {
        slope: 2.0,
        ..counting()
    };
    assert_eq!(read_labels(&nifti(&scaled)), Err(NiftiError::Scaled));
    let mut shifted = nifti(&counting());
    shifted[116..120].copy_from_slice(&5.0_f32.to_le_bytes());
    assert_eq!(
        read_labels(&shifted),
        Err(NiftiError::Scaled),
        "an intercept is scaling too"
    );
}

#[test]
fn numbers_that_are_not_whole_numbers_from_nought_up_are_not_labels() {
    let fraction = NiftiOptions {
        kind: 16,
        ..counting()
    };
    let mut bytes = nifti(&fraction);
    bytes[352..356].copy_from_slice(&1.5_f32.to_le_bytes());
    assert_eq!(read_labels(&bytes), Err(NiftiError::NotWhole));
    bytes[352..356].copy_from_slice(&f32::NAN.to_le_bytes());
    assert_eq!(read_labels(&bytes), Err(NiftiError::NotWhole));
    let signed = NiftiOptions {
        kind: 4,
        ..counting()
    };
    let mut bytes = nifti(&signed);
    bytes[352..354].copy_from_slice(&(-1_i16).to_le_bytes());
    assert_eq!(read_labels(&bytes), Err(NiftiError::NotWhole));
}

#[test]
fn a_file_that_does_not_say_where_it_lies_is_refused() {
    let nowhere = NiftiOptions {
        srow: None,
        ..counting()
    };
    assert_eq!(read_labels(&nifti(&nowhere)), Err(NiftiError::NoPlace));
    let mut flat = nifti(&counting());
    // An sform that squashes everything on to one plane cannot be turned round.
    flat[312..328].fill(0);
    assert_eq!(read_labels(&flat), Err(NiftiError::NoPlace));
}

#[test]
fn a_file_too_large_or_cut_short_is_refused_without_being_read_through() {
    let mut huge = nifti(&counting());
    for at in [42, 44, 46] {
        put(&mut huge, at, 4000);
    }
    assert_eq!(read_labels(&huge), Err(NiftiError::TooLarge));
    let whole = nifti(&counting());
    let short = &whole[..whole.len() - 1];
    assert_eq!(read_labels(short), Err(NiftiError::CutShort));
    // A volume of numbers with no pattern, so that half its gzip is well into its values.
    let scattered = (0..8000_u32).map(|at| at.wrapping_mul(2_654_435_761) >> 20);
    let noisy = NiftiOptions {
        dims: [20, 20, 20],
        ids: scattered.collect(),
        ..counting()
    };
    let zipped = gzipped(&nifti(&noisy));
    assert!(read_labels(&zipped).is_ok());
    assert_eq!(
        read_labels(&zipped[..zipped.len() / 2]),
        Err(NiftiError::CutShort)
    );
    assert_eq!(
        read_labels(&zipped[..12]),
        Err(NiftiError::NotNifti),
        "not even a header"
    );
    let mut no_size = nifti(&counting());
    put(&mut no_size, 42, 0);
    assert_eq!(read_labels(&no_size), Err(NiftiError::NotNifti));
}

#[test]
fn every_refusal_is_a_sentence_a_person_can_read() {
    let refusals = [
        NiftiError::NotNifti,
        NiftiError::Nifti2,
        NiftiError::TwoFiles,
        NiftiError::NotThreeD,
        NiftiError::Kind(128),
        NiftiError::Scaled,
        NiftiError::NotWhole,
        NiftiError::NoPlace,
        NiftiError::TooLarge,
        NiftiError::CutShort,
    ];
    for refusal in refusals {
        let said = refusal.to_string();
        assert!(said.ends_with('.') && said.len() > 20, "{said}");
        for jargon in ["Error", "u16", "i16", "f32", "::", "sform", "qform"] {
            assert!(!said.contains(jargon), "{said}");
        }
    }
}
