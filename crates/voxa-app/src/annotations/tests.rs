use super::*;

fn fine() -> Affine<Voxel, Patient> {
    Affine::from_columns([0.5, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 3.0], [0.0; 3])
}

fn ruler(to: [f64; 3]) -> Shape {
    Shape::Ruler { a: [0.0; 3], b: to }
}

#[test]
fn a_ruler_gives_its_length_and_how_exactly_it_can_be_known() {
    let across = measured(&ruler([5.0, 0.0, 0.0]), Some(&fine()), Scale::Patient);
    assert_eq!((across.kind, across.value.as_str()), ("Ruler", "5.0 mm"));
    let known = "Its ends can be placed to about one voxel: ± 0.5 mm along this line.";
    assert_eq!(across.detail, known);
    // At a slant across the slice a layer of voxels is met every 0.625 mm of the line.
    let slanted = measured(&ruler([3.0, 4.0, 0.0]), Some(&fine()), Scale::Patient);
    assert_eq!(slanted.value, "5.0 mm");
    assert!(slanted.detail.contains("± 0.6 mm"), "{}", slanted.detail);
    let through = measured(&ruler([0.0, 0.0, 12.0]), Some(&fine()), Scale::Patient);
    assert_eq!(through.value, "12.0 mm");
    assert!(through.detail.contains("± 3.0 mm"), "{}", through.detail);
    assert_eq!(
        measured(&ruler([3.0, 4.0, 0.0]), None, Scale::Patient).detail,
        ""
    );
    assert_eq!(
        measured(&ruler([0.0; 3]), Some(&fine()), Scale::Patient).detail,
        "",
        "no length, no way"
    );
}

#[test]
fn an_angle_is_given_in_degrees_unless_an_arm_has_no_length() {
    let (a, vertex) = ([1.0, 0.0, 0.0], [0.0; 3]);
    let right = Shape::Angle {
        a,
        vertex,
        b: [0.0, 2.0, 0.0],
    };
    assert_eq!(measured(&right, None, Scale::Patient).value, "90.0°");
    let flat = Shape::Angle {
        a,
        vertex,
        b: vertex,
    };
    assert_eq!(
        (
            measured(&flat, None, Scale::Patient).kind,
            measured(&flat, None, Scale::Patient).value.as_str()
        ),
        ("Angle", "no angle")
    );
}

#[test]
fn a_region_gives_what_the_scan_holds_inside_it() {
    let region = |stats| Shape::Region {
        centre: [0.0; 3],
        across: [1.0, 0.0, 0.0],
        down: [0.0, 1.0, 0.0],
        radii_mm: [5.0, 5.0],
        stats,
    };
    let stats = RegionStats {
        mean: 312.04,
        sd: 41.46,
        min: 201.2,
        max: 387.6,
        area_mm2: 180.0,
        samples: 412,
    };
    let inside = measured(&region(Some(stats)), None, Scale::Patient);
    assert_eq!(inside.value, "mean 312.0, SD 41.5");
    assert_eq!(inside.detail, "min 201, max 388, area 1.8 cm², 412 samples");
    let outside = measured(&region(None), None, Scale::Patient);
    assert_eq!(
        (outside.value.as_str(), outside.detail.as_str()),
        ("outside the scan", "area 78.5 mm²")
    );
}

#[test]
fn an_area_is_in_square_millimetres_until_it_is_a_square_centimetre() {
    assert_eq!(area_in_words(78.54), "78.5 mm²");
    assert_eq!(area_in_words(250.0), "2.5 cm²");
    assert_eq!(
        area_in_words(99.96),
        "100.0 mm²",
        "said as it rounds, just short of one"
    );
}

#[test]
fn a_marker_says_where_it_is_from_the_scanners_origin() {
    let marker = Shape::Marker {
        at: [-12.0, 4.5, 30.2],
    };
    let said = measured(&marker, None, Scale::Patient);
    assert_eq!(
        (said.kind, said.value.as_str()),
        ("Marker", "R 12.0, P 4.5, S 30.2 mm")
    );
    let other = Shape::Marker {
        at: [12.0, -4.5, -30.2],
    };
    assert_eq!(
        measured(&other, None, Scale::Patient).value,
        "L 12.0, A 4.5, I 30.2 mm"
    );
}

#[test]
fn an_annotation_is_placed_by_its_points_the_first_being_where_to_look() {
    let angle = Shape::Angle {
        a: [1.0; 3],
        vertex: [2.0; 3],
        b: [3.0; 3],
    };
    assert_eq!(angle.points(), [[2.0; 3], [1.0; 3], [3.0; 3]]);
    assert_eq!(ruler([1.0; 3]).points(), [[0.0; 3], [1.0; 3]]);
    let loaded = Annotations::loaded(vec![Annotation {
        id: 7,
        label: "kept".to_owned(),
        shape: ruler([1.0; 3]),
        scale: Scale::Patient,
    }]);
    assert!(!loaded.can_undo() && !loaded.is_full());
    let mut list = loaded;
    assert!(list.add(Shape::Marker { at: [0.0; 3] }, Scale::Patient));
    assert_eq!(list.items[1].id, 8, "numbers carry on from what was kept");
}

#[test]
fn a_marker_on_a_picture_with_no_place_in_the_patient_says_where_it_is_in_the_picture() {
    let marker = Shape::Marker {
        at: [12.0, 4.5, 2.0],
    };
    let said = measured(&marker, None, Scale::PixelSpacing);
    assert_eq!(said.value, "12.0 mm across, 4.5 mm down, slice 3");
    let from = "From the middle of the picture's first pixel, as its file holds it.";
    assert_eq!(said.detail, from);
    assert_eq!(
        said.shown(),
        "12.0 mm across, 4.5 mm down, slice 3 (from the file's pixel spacing)"
    );
    // Lengths and angles are measured the same way; they only say where they come from.
    let ruler = measured(&ruler([3.0, 4.0, 0.0]), None, Scale::PixelSpacing);
    assert_eq!(ruler.value, "5.0 mm");
    assert_eq!(ruler.shown(), "5.0 mm (from the file's pixel spacing)");
    let in_the_patient = measured(&marker, None, Scale::Patient);
    assert_eq!(in_the_patient.shown(), in_the_patient.value);
}
