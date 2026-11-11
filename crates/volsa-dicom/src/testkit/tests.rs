use std::io::Cursor;

use dicom_dictionary_std::tags;

use super::*;

#[test]
fn a_classic_slice_parses_as_dicom() {
    let bytes = classic_slice(&SliceOptions {
        rows: 6,
        cols: 8,
        ..SliceOptions::default()
    });
    let obj = dicom_object::from_reader(Cursor::new(bytes)).expect("parses");
    assert_eq!(
        obj.element(tags::ROWS)
            .expect("rows")
            .to_int::<u16>()
            .expect("int"),
        6
    );
    assert_eq!(
        obj.element(tags::COLUMNS)
            .expect("cols")
            .to_int::<u16>()
            .expect("int"),
        8
    );
    let position = obj.element(tags::IMAGE_POSITION_PATIENT).expect("position");
    assert_eq!(
        position.to_multi_float64().expect("floats"),
        vec![-100.0, -120.0, 0.0]
    );
    let pixels = obj
        .element(tags::PIXEL_DATA)
        .expect("pixels")
        .to_bytes()
        .expect("bytes");
    assert_eq!(pixels.len(), 6 * 8 * 2);
    assert_eq!(obj.meta().transfer_syntax(), EXPLICIT_VR_LITTLE_ENDIAN);
}

#[test]
fn an_enhanced_series_parses_with_one_group_per_frame() {
    let frames = (0..24)
        .map(|i| EnhancedFrame::at([-100.0, -120.0, f64::from(i) * 5.0]))
        .collect();
    let bytes = enhanced_series(&EnhancedOptions {
        common: SliceOptions::default(),
        frames,
    });
    let obj = dicom_object::from_reader(Cursor::new(bytes)).expect("parses");
    assert_eq!(
        obj.element(tags::NUMBER_OF_FRAMES)
            .expect("frames")
            .to_int::<u32>()
            .expect("int"),
        24
    );
    let groups = obj
        .element(tags::PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)
        .expect("per-frame groups");
    assert_eq!(groups.items().expect("is a sequence").len(), 24);
    let pixels = obj
        .element(tags::PIXEL_DATA)
        .expect("pixels")
        .to_bytes()
        .expect("bytes");
    assert_eq!(pixels.len(), 24 * 4 * 4 * 2);
}

#[test]
fn a_relabelled_file_names_the_other_transfer_syntax() {
    let made_up = "1.2.840.10008.9.9.9";
    let bytes = classic_slice(&SliceOptions {
        transfer_syntax: made_up.to_owned(),
        ..SliceOptions::default()
    });
    assert!(
        bytes
            .windows(made_up.len())
            .any(|w| w == made_up.as_bytes())
    );
    assert!(
        !bytes
            .windows(19)
            .any(|w| w == EXPLICIT_VR_LITTLE_ENDIAN.as_bytes())
    );
}

#[test]
fn dropped_pixels_shorten_the_pixel_data() {
    let bytes = classic_slice(&SliceOptions {
        drop_pixels: 5,
        ..SliceOptions::default()
    });
    let obj = dicom_object::from_reader(Cursor::new(bytes)).expect("parses");
    let pixels = obj
        .element(tags::PIXEL_DATA)
        .expect("pixels")
        .to_bytes()
        .expect("bytes");
    assert_eq!(pixels.len(), (16 - 5) * 2);
}

#[test]
fn a_memory_source_yields_names_in_order_and_reads_by_name() {
    let mut source = MemorySource::new(vec![("b".to_owned(), vec![2]), ("a".to_owned(), vec![1])]);
    let names: Vec<String> = source.entries().map(|e| e.expect("entry").name).collect();
    assert_eq!(names, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(source.read("b").expect("reads"), vec![2]);
    assert!(matches!(source.read("c"), Err(VoxaError::MissingEntry(_))));
}
