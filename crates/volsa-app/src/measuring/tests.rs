//! In whose millimetres a scan is measured on its slices, from what its files say; what a
//! region drawn on a slice holds; and the measurements kept by 0.6.0.

use volsa_core::{Point, SeriesProblem};
use volsa_dicom::Frame;
use volsa_dicom::testkit::{SliceOptions, classic_slice};
use volsa_volume::RegionStats;

use super::*;
use crate::action::Action;
use crate::annotations::{
    ANNOTATIONS_VERSION, Shape, load_annotations, measured, save_annotations,
};
use crate::reducer::reduce;
use crate::testing::{
    LOCALISER, THICK_SCAN, id, indexed, showing_a_picture, shown, slice_options, thick_scan_ready,
    three_series, with_odd_series,
};

fn xyz(point: Point<Patient>) -> [f64; 3] {
    [point.x, point.y, point.z]
}

/// A state with these files indexed and `series` shown in the first viewport, its slice not
/// yet decoded.
fn opened(index: volsa_dicom::Index, series: &str) -> AppState {
    let mut state = AppState::default();
    reduce(&mut state, Action::IndexProgress(index));
    let (viewport, series) = (0, id(series));
    reduce(&mut state, Action::SelectSeries { viewport, series });
    state
}

#[test]
fn a_scan_with_geometry_is_measured_in_the_patients_millimetres() {
    let state = thick_scan_ready();
    let info = &state.series(&id(THICK_SCAN)).expect("listed").info;
    let at = placement_of(info).expect("measured");
    assert_eq!(at.scale, Scale::Patient);
    assert_eq!(Some(at.placement), info.voxel_to_patient);
    assert_eq!(why_not_measurable(&state), None);
    assert_eq!(scale_now(&state), Some(Scale::Patient));
    assert_eq!(scene_placement(&state), Some(at));
}

#[test]
fn a_scan_with_spacing_only_is_measured_within_the_picture() {
    let state = showing_a_picture(Some([0.5, 0.5]), None);
    let info = state.shown_info(0).expect("shown");
    assert!(info.spacing_given && info.voxel_to_patient.is_none());
    let at = placement_of(info).expect("measured within the picture");
    assert_eq!(at.scale, Scale::PixelSpacing);
    let on_row = |col: f64| xyz(at.placement.apply(Point::new(col, 3.0, 0.0)));
    let ruler = Shape::Ruler {
        a: on_row(2.0),
        b: on_row(12.0),
    };
    let said = measured(&ruler, Some(&at.placement), at.scale);
    assert_eq!(said.value, "5.0 mm", "ten columns 0.5 mm apart");
    assert_eq!(said.shown(), "5.0 mm (from the file's pixel spacing)");
    assert_eq!(why_not_measurable(&state), None);
    assert_eq!(scale_now(&state), Some(Scale::PixelSpacing));
    // The file gives the distance between rows first, then between columns.
    let state = showing_a_picture(Some([0.25, 0.5]), None);
    let at = placement_of(state.shown_info(0).expect("shown")).expect("measured");
    let wanted = Affine::from_columns([0.5, 0.0, 0.0], [0.0, 0.25, 0.0], [0.0, 0.0, 1.0], [0.0; 3]);
    assert_eq!(at.placement, wanted);
}

#[test]
fn a_scan_without_spacing_is_not_measured_and_says_why() {
    assert_eq!(
        NO_SCALE,
        "Measuring needs a scan with a scale in its files."
    );
    // A detector's spacing, as an X-ray gives, says nothing of the body's size.
    for detector in [None, Some([0.2, 0.2])] {
        let state = showing_a_picture(None, detector);
        let info = state.shown_info(0).expect("shown");
        assert!(!info.spacing_given, "{detector:?}");
        assert_eq!(placement_of(info), None, "{detector:?}");
        assert_eq!(why_not_measurable(&state), Some(NO_SCALE), "{detector:?}");
        assert_eq!(scale_now(&state), None);
    }
    let nothing = AppState::default();
    assert_eq!(why_not_measurable(&nothing), Some(NO_SCALE), "nothing open");
}

#[test]
fn a_spacing_that_is_not_a_length_above_zero_is_no_scale() {
    for spacing in [[0.0, 0.5], [0.5, 0.0], [-0.5, 0.5], [0.5, -0.5]] {
        let state = showing_a_picture(Some(spacing), None);
        let info = state.shown_info(0).expect("shown");
        assert!(info.spacing_given, "the file gave one: {spacing:?}");
        assert_eq!(placement_of(info), None, "{spacing:?}");
        assert_eq!(why_not_measurable(&state), Some(NO_SCALE), "{spacing:?}");
    }
    for odd in [0.0, -0.5, f64::NAN, f64::INFINITY] {
        for spacing_mm in [[odd, 0.5, 1.0], [0.5, odd, 1.0]] {
            let info = SeriesInfo {
                rows: 4,
                cols: 4,
                spacing_mm,
                spacing_given: true,
                ..SeriesInfo::default()
            };
            assert_eq!(placement_of(&info), None, "{spacing_mm:?}");
            let placed = SeriesInfo {
                voxel_to_patient: Some(Affine::identity()),
                ..info
            };
            assert_eq!(placement_of(&placed), None, "with geometry: {spacing_mm:?}");
        }
    }
}

#[test]
fn slices_of_several_sizes_or_planes_are_not_measured_and_say_why() {
    let mut state = opened(with_odd_series(), LOCALISER);
    shown(&mut state);
    let info = state.shown_info(0).expect("shown");
    assert!(info.problems.contains(&SeriesProblem::MixedOrientations));
    assert!(info.spacing_given);
    assert_eq!(placement_of(info), None);
    assert_eq!(why_not_measurable(&state), Some(NO_ONE_SCALE));
    // One slice four pixels across and the next eight: no one grid for the two.
    let files = [4_u16, 8].iter().zip(0..).map(|(side, place)| {
        let options = slice_options("1.2.3.10", 10, place, *side);
        (format!("S10/I{place}.dcm"), classic_slice(&options))
    });
    let mut state = opened(indexed(files.collect()), "1.2.3.10");
    shown(&mut state);
    let info = state.shown_info(0).expect("shown");
    assert!(info.problems.contains(&SeriesProblem::MixedSizes));
    assert_eq!(placement_of(info), None);
    assert_eq!(why_not_measurable(&state), Some(NO_ONE_SCALE));
}

#[test]
fn a_slice_is_measured_on_once_it_is_shown() {
    let mut state = opened(three_series(), THICK_SCAN);
    assert_eq!(why_not_measurable(&state), Some(NOT_SHOWN));
    assert_eq!(scale_now(&state), None);
    shown(&mut state);
    assert_eq!(
        why_not_measurable(&state),
        None,
        "the volume is still being built"
    );
    assert_eq!(scale_now(&state), Some(Scale::Patient));
}

#[test]
fn a_scale_is_named_as_the_saved_measurements_name_it() {
    assert_eq!(Scale::default(), Scale::Patient);
    assert_eq!(Scale::Patient.name(), "patient");
    assert_eq!(Scale::PixelSpacing.name(), "pixel spacing");
    assert_eq!(Scale::Patient.said_after(), "");
    let said = " (from the file's pixel spacing)";
    assert_eq!(Scale::PixelSpacing.said_after(), said);
    for scale in [Scale::Patient, Scale::PixelSpacing] {
        let written = serde_json::to_string(&scale).expect("written");
        assert_eq!(written, format!("\"{}\"", scale.name()));
    }
}

/// A 16 x 16 slice of 100s with a block of 7s in columns and rows 4 to 11.
fn sevens_inside() -> Frame {
    let block = |at: usize| (4..12).contains(&(at % 16)) && (4..12).contains(&(at / 16));
    let values = (0..256)
        .map(|at| if block(at) { 7.0 } else { 100.0 })
        .collect();
    Frame {
        rows: 16,
        cols: 16,
        values,
        colour: None,
    }
}

/// What a region about pixel (`col`, `row`) of slice `slice` holds, its half-widths `radii`.
fn region_at(
    at: &SlicePlacement,
    slice: usize,
    (col, row): (f64, f64),
    radii: [f64; 2],
) -> Option<RegionStats> {
    let frame = sevens_inside();
    let plane = slice_plane(at, slice, [16, 16]);
    let centre = xyz(at.placement.apply(Point::new(col, row, slice as f64)));
    let (across, down) = (plane.across, plane.down);
    frame_ellipse_stats((&frame).into(), at, slice, centre, radii, across, down)
}

#[test]
fn a_region_on_a_slice_takes_its_values_from_that_slice() {
    let picture = SlicePlacement {
        placement: Affine::from_columns(
            [0.5, 0.0, 0.0],
            [0.0, 0.5, 0.0],
            [0.0, 0.0, 1.0],
            [0.0; 3],
        ),
        scale: Scale::PixelSpacing,
    };
    let tilted = SlicePlacement {
        placement: Affine::from_columns(
            [0.0, 0.9, 0.0],
            [0.0, 0.0, -0.9],
            [5.0, 0.0, 0.0],
            [-40.0, 3.0, 9.0],
        ),
        scale: Scale::Patient,
    };
    for (at, slice) in [(picture, 0), (tilted, 6)] {
        let inside = region_at(&at, slice, (7.5, 7.5), [1.5, 1.0]).expect("on the slice");
        assert_eq!((inside.mean, inside.sd), (7.0, 0.0), "{at:?}");
        assert_eq!((inside.min, inside.max), (7.0, 7.0));
        assert!(inside.samples > 0);
        let area = std::f64::consts::PI * 1.5;
        assert!(
            (inside.area_mm2 - area).abs() < 1.0e-9,
            "{}",
            inside.area_mm2
        );
        // Over the edge of the block it holds both.
        let edge = region_at(&at, slice, (3.5, 7.5), [1.0, 1.0]).expect("on the slice");
        assert_eq!((edge.min, edge.max), (7.0, 100.0));
        assert!(edge.mean > 7.0 && edge.mean < 100.0);
        // The smallest region the tools make still takes a sample.
        let least = region_at(&at, slice, (7.0, 7.0), [0.5, 0.5]).expect("on the slice");
        assert!(least.samples > 0);
        // Wholly off the slice it holds nothing; with no width it is no region.
        assert_eq!(region_at(&at, slice, (-40.0, 7.5), [2.0, 2.0]), None);
        assert_eq!(region_at(&at, slice, (7.5, 7.5), [0.0, 1.0]), None);
        assert_eq!(region_at(&at, slice, (7.5, 7.5), [f64::NAN, 1.0]), None);
    }
}

#[test]
fn a_slice_plane_lies_along_its_rows_and_columns_through_its_middle() {
    let at = SlicePlacement {
        placement: Affine::from_columns(
            [0.0, 0.9, 0.0],
            [0.0, 0.0, -0.9],
            [5.0, 0.0, 0.0],
            [-40.0, 3.0, 9.0],
        ),
        scale: Scale::Patient,
    };
    let plane = slice_plane(&at, 6, [16, 8]);
    assert_eq!(
        (plane.across, plane.down),
        ([0.0, 1.0, 0.0], [0.0, 0.0, -1.0])
    );
    let middle = at.placement.apply(Point::new(7.5, 3.5, 6.0));
    assert_eq!(plane.centre, middle);
    // A slice's next neighbour, 5 mm along the normal, is not on it; the slice itself is.
    let half = half_slice(&at, &plane);
    assert!((half - 2.5).abs() < 1.0e-5, "{half}");
}

#[test]
fn a_plain_stack_says_so_in_the_list() {
    // Slices 5 mm and then 10 mm apart.
    let files = [0.0, 5.0, 15.0].iter().zip(0..).map(|(z, place)| {
        let options = SliceOptions {
            position: Some([0.0, 0.0, *z]),
            ..slice_options("1.2.3.11", 11, place, 4)
        };
        (format!("S11/I{place}.dcm"), classic_slice(&options))
    });
    let mut state = opened(indexed(files.collect()), "1.2.3.11");
    shown(&mut state);
    let info = state.shown_info(0).expect("shown");
    assert!(info.problems.contains(&SeriesProblem::UnevenSpacing));
    assert_eq!(placement_of(info).map(|at| at.scale), Some(Scale::Patient));
    assert_eq!(plain_stack_note(&state), Some(PLAIN_STACK));
    assert_eq!(plain_stack_note(&thick_scan_ready()), None);
    let picture = showing_a_picture(Some([0.5, 0.5]), None);
    assert_eq!(
        plain_stack_note(&picture),
        None,
        "it says where its scale comes from"
    );
}

#[test]
fn measurements_kept_by_0_6_0_still_open() {
    assert_eq!(ANNOTATIONS_VERSION, 2);
    let folder = tempfile::tempdir().expect("a temporary folder");
    let file = folder.path().join("annotations.json");
    // As 0.6.0 wrote it: version 1, with no scale.
    let kept = r#"{
  "version": 1,
  "series": {
    "0123456789abcdef": [
      {
        "id": 3,
        "label": "Ruler 3",
        "shape": {
          "Ruler": {
            "a": [0.0, 0.0, 10.0],
            "b": [3.0, -4.0, 10.0]
          }
        }
      }
    ]
  }
}"#;
    std::fs::write(&file, kept).expect("writes");
    let read = load_annotations(folder.path(), "0123456789abcdef");
    let [ruler] = &read[..] else {
        panic!("{read:?}");
    };
    assert_eq!((ruler.id, ruler.label.as_str()), (3, "Ruler 3"));
    assert_eq!(
        ruler.scale,
        Scale::Patient,
        "measured in a 3D view, in the patient"
    );
    let shape = Shape::Ruler {
        a: [0.0, 0.0, 10.0],
        b: [3.0, -4.0, 10.0],
    };
    assert_eq!(ruler.shape, shape);
    // Written again for another series, it keeps the ruler, now saying its scale. Nothing in the
    // file is in a picture's own millimetres, so it is still a file 0.6.0 reads.
    save_annotations(folder.path(), "another", &read).expect("saves");
    let text = std::fs::read_to_string(&file).expect("reads");
    assert!(text.contains("\"version\": 1"), "{text}");
    assert!(text.contains("\"scale\": \"patient\""), "{text}");
    assert_eq!(load_annotations(folder.path(), "0123456789abcdef"), read);
}

#[test]
fn a_picture_in_colour_is_measured_but_has_no_values_for_a_region() {
    use crate::testing::{COLOUR, colour_files};
    let mut state = opened(indexed(colour_files()), COLOUR);
    shown(&mut state);
    assert!(state.shown_info(0).is_some_and(|info| info.colour));
    assert_eq!(
        why_not_measurable(&state),
        None,
        "lengths, angles and markers"
    );
    assert_eq!(why_no_region(&state), Some(IN_COLOUR));
    assert_eq!(why_no_region(&thick_scan_ready()), None);
    let nothing = AppState::default();
    assert_eq!(
        why_no_region(&nothing),
        Some(NO_SCALE),
        "the first reason first"
    );
}
