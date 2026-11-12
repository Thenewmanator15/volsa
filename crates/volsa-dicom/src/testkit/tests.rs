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
fn a_detectors_spacing_and_a_rescale_type_are_written_when_given() {
    let opts = SliceOptions {
        imager_pixel_spacing: Some([0.2, 0.25]),
        rescale_type: Some("HU".to_owned()),
        ..SliceOptions::default()
    };
    let obj = dicom_object::from_reader(Cursor::new(classic_slice(&opts))).expect("parses");
    let detector = obj.element(tags::IMAGER_PIXEL_SPACING).expect("detector");
    assert_eq!(detector.to_multi_float64().expect("floats"), [0.2, 0.25]);
    let kind = obj.element(tags::RESCALE_TYPE).expect("rescale type");
    assert_eq!(kind.to_str().expect("text").trim(), "HU");

    let plain = classic_slice(&SliceOptions::default());
    let obj = dicom_object::from_reader(Cursor::new(plain)).expect("parses");
    assert!(obj.element(tags::IMAGER_PIXEL_SPACING).is_err());
    assert!(obj.element(tags::RESCALE_TYPE).is_err());
}

#[test]
fn a_report_parses_as_dicom_with_its_content_in_order_and_no_image() {
    let content = [
        ReportItem::container(
            "Part",
            vec![ReportItem::text("Comment", "A made-up remark.")],
        ),
        ReportItem::number("Width", "12", "mm"),
    ];
    let bytes = structured_report(&DocumentOptions::default(), &content);
    let obj = dicom_object::from_reader(Cursor::new(bytes)).expect("parses");
    assert_eq!(
        obj.element(tags::SOP_CLASS_UID)
            .expect("class")
            .to_str()
            .expect("text")
            .trim_end_matches('\0'),
        "1.2.840.10008.5.1.4.1.1.88.11"
    );
    let items = obj
        .element(tags::CONTENT_SEQUENCE)
        .expect("content")
        .items()
        .expect("items");
    assert_eq!(items.len(), 2);
    let kinds: Vec<String> = items
        .iter()
        .map(|item| {
            item.element(tags::VALUE_TYPE)
                .expect("type")
                .to_str()
                .expect("text")
                .trim()
                .to_owned()
        })
        .collect();
    assert_eq!(kinds, ["CONTAINER", "NUM"]);
    assert!(obj.element(tags::ROWS).is_err() && obj.element(tags::PIXEL_DATA).is_err());
}

#[test]
fn a_pdf_is_written_padded_to_an_even_length_and_a_class_can_be_overridden() {
    let odd = b"%PDF-1.4 a made-up page";
    assert_eq!(odd.len() % 2, 1, "the test needs a document of odd length");
    let bytes = encapsulated_pdf(&DocumentOptions::default(), odd);
    let obj = dicom_object::from_reader(Cursor::new(bytes)).expect("parses");
    let held = obj.element(tags::ENCAPSULATED_DOCUMENT).expect("document");
    let held = held.to_bytes().expect("bytes");
    assert_eq!(held.len(), odd.len() + 1);
    assert_eq!(held.get(..odd.len()), Some(odd.as_slice()));
    assert_eq!(held.last(), Some(&0));
    let length = obj
        .element(tags::ENCAPSULATED_DOCUMENT_LENGTH)
        .expect("length");
    assert_eq!(
        length.to_int::<usize>().expect("int"),
        odd.len(),
        "without the pad"
    );

    let other = DocumentOptions {
        sop_class: Some("1.2.840.10008.5.1.4.1.1.11.1".to_owned()),
        ..DocumentOptions::default()
    };
    let obj =
        dicom_object::from_reader(Cursor::new(structured_report(&other, &[]))).expect("parses");
    let class = obj.element(tags::SOP_CLASS_UID).expect("class");
    assert_eq!(
        class.to_str().expect("text").trim_end_matches('\0'),
        "1.2.840.10008.5.1.4.1.1.11.1"
    );
}

#[test]
fn a_memory_source_yields_names_in_order_and_reads_by_name() {
    let mut source = MemorySource::new(vec![("b".to_owned(), vec![2]), ("a".to_owned(), vec![1])]);
    let names: Vec<String> = source.entries().map(|e| e.expect("entry").name).collect();
    assert_eq!(names, vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(source.read("b").expect("reads"), vec![2]);
    assert!(matches!(source.read("c"), Err(VolsaError::MissingEntry(_))));
}
