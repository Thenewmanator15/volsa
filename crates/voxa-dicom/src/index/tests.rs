use proptest::prelude::*;
use voxa_core::{Point, SeriesId, SeriesInfo, SeriesProblem};

use super::*;
use crate::testkit::{
    EnhancedFrame, EnhancedOptions, MemorySource, SliceOptions, classic_slice, enhanced_series,
};

/// A stack of `count` slices `gap` mm apart, stored under names whose order is not their
/// position order.
pub(super) fn stack(count: i32, gap: f64, base: &SliceOptions) -> Vec<(String, Vec<u8>)> {
    (0..count)
        .map(|i| {
            let place = (i * 7) % count; // a shuffle: name order visits positions out of order
            let opts = SliceOptions {
                instance_number: place + 1,
                position: Some([-100.0, -120.0, f64::from(place) * gap]),
                ..base.clone()
            };
            (
                format!("IMAGES/S{}/I{i:04}", base.series_number),
                classic_slice(&opts),
            )
        })
        .collect()
}

pub(super) fn indexed(files: Vec<(String, Vec<u8>)>) -> Index {
    index(&mut MemorySource::new(files), &mut |_| {}).expect("indexing does not fail")
}

pub(super) fn heights(
    index: &Index,
    series: &IndexedSeries,
    source_files: &[(String, Vec<u8>)],
) -> Vec<i32> {
    // The instance number was set from the position, so it shows the order the slices ended up in.
    let _ = index;
    series
        .slices
        .iter()
        .map(|slice| {
            let bytes = &source_files
                .iter()
                .find(|(name, _)| *name == slice.entry)
                .expect("entry exists")
                .1;
            let obj = dicom_object::from_reader(std::io::Cursor::new(bytes)).expect("parses");
            obj.element(dicom_dictionary_std::tags::INSTANCE_NUMBER)
                .expect("number")
                .to_int::<i32>()
                .expect("int")
        })
        .collect()
}

#[test]
fn index_puts_shuffled_slices_in_position_order_with_their_geometry() {
    let files = stack(30, 5.0, &SliceOptions::default());
    let index = indexed(files.clone());
    assert_eq!(index.studies.len(), 1);
    assert_eq!(index.series.len(), 1);
    let series = &index.series[0];
    assert_eq!(
        heights(&index, series, &files),
        (1..=30).collect::<Vec<_>>()
    );
    assert!(series.slices.iter().all(|s| s.frame.is_none()));
    assert_eq!(series.info.slices, 30);
    assert_eq!((series.info.rows, series.info.cols), (4, 4));
    assert_eq!(series.info.spacing_mm, [0.9, 0.9, 5.0]);
    assert_eq!(series.info.problems, Vec::new());
    assert_eq!(series.info.modality, "MR");
    assert_eq!(series.info.description, "synthetic");
    let to_patient = series.info.voxel_to_patient.expect("has geometry");
    let p = to_patient.apply(Point::new(10.0, 20.0, 3.0));
    assert_eq!((p.x, p.y, p.z), (-91.0, -102.0, 15.0));
    assert_eq!(index.studies[0].series, vec![series.info.clone()]);
}

#[test]
fn index_keeps_patient_details_on_the_study_only() {
    let index = indexed(stack(3, 5.0, &SliceOptions::default()));
    let study = &index.studies[0];
    assert_eq!(study.patient_name.as_deref(), Some("Phantom^Test"));
    assert_eq!(study.patient_id.as_deref(), Some("PHANTOM-001"));
    assert_eq!(study.date.as_deref(), Some("20200102"));
    assert_eq!(study.description, "Synthetic study");
}

#[test]
fn index_orders_studies_by_date_and_series_by_number() {
    let later = SliceOptions {
        study_uid: "1.2.3.100".to_owned(),
        study_date: Some("20210830".to_owned()),
        series_uid: "1.2.3.100.1".to_owned(),
        series_number: 5,
        ..SliceOptions::default()
    };
    let earlier = SliceOptions {
        study_uid: "1.2.3.900".to_owned(),
        study_date: Some("20190412".to_owned()),
        series_uid: "1.2.3.900.7".to_owned(),
        series_number: 7,
        ..SliceOptions::default()
    };
    let earlier_first_series = SliceOptions {
        series_uid: "1.2.3.900.2".to_owned(),
        series_number: 2,
        ..earlier.clone()
    };
    let mut files = stack(3, 5.0, &later);
    files.extend(stack(3, 5.0, &earlier));
    files.extend(stack(3, 5.0, &earlier_first_series));
    let index = indexed(files);
    let dates: Vec<_> = index
        .studies
        .iter()
        .map(|s| s.date.clone().expect("dated"))
        .collect();
    assert_eq!(dates, vec!["20190412".to_owned(), "20210830".to_owned()]);
    let numbers: Vec<i32> = index.series.iter().map(|s| s.info.number).collect();
    assert_eq!(
        numbers,
        vec![2, 7, 5],
        "series follow their study, then their number"
    );
    assert_eq!(index.studies[0].series.len(), 2);
}

#[test]
fn index_flags_a_gap_in_the_stack() {
    let mut files = stack(10, 5.0, &SliceOptions::default());
    files.retain(|(name, _)| !name.ends_with("I0003")); // removes one slice from the middle
    let index = indexed(files);
    assert_eq!(
        index.series[0].info.problems,
        vec![SeriesProblem::UnevenSpacing]
    );
    assert!(
        index.series[0].info.has_geometry(),
        "an uneven stack still knows where its slices are"
    );
}

#[test]
fn index_flags_slices_that_share_a_position() {
    let base = SliceOptions::default();
    let mut files = stack(6, 5.0, &base);
    let twin = SliceOptions {
        instance_number: 99,
        position: Some([-100.0, -120.0, 10.0]),
        ..base
    };
    files.push(("IMAGES/S1/twin".to_owned(), classic_slice(&twin)));
    let index = indexed(files);
    assert_eq!(
        index.series[0].info.problems,
        vec![SeriesProblem::RepeatedPositions]
    );
    assert_eq!(index.series[0].info.slices, 7);
}

#[test]
fn index_flags_a_slice_of_a_different_size() {
    let base = SliceOptions::default();
    let mut files = stack(5, 5.0, &base);
    let odd = SliceOptions {
        rows: 8,
        instance_number: 6,
        position: Some([-100.0, -120.0, 25.0]),
        ..base
    };
    files.push(("IMAGES/S1/odd".to_owned(), classic_slice(&odd)));
    let index = indexed(files);
    assert_eq!(
        index.series[0].info.problems,
        vec![SeriesProblem::MixedSizes]
    );
}

#[test]
fn index_sorts_by_instance_number_when_positions_are_missing() {
    let base = SliceOptions {
        position: None,
        ..SliceOptions::default()
    };
    let files: Vec<(String, Vec<u8>)> = [3, 1, 2]
        .iter()
        .enumerate()
        .map(|(i, number)| {
            let opts = SliceOptions {
                instance_number: *number,
                ..base.clone()
            };
            (format!("f{i}"), classic_slice(&opts))
        })
        .collect();
    let index = indexed(files.clone());
    let series = &index.series[0];
    assert_eq!(series.info.problems, vec![SeriesProblem::MissingGeometry]);
    assert!(series.info.voxel_to_patient.is_none());
    assert_eq!(heights(&index, series, &files), vec![1, 2, 3]);
    assert_eq!(
        series.info.spacing_mm,
        [0.9, 0.9, 5.0],
        "spacing falls back to the stated thickness"
    );
}

#[test]
fn index_reads_an_enhanced_file_as_one_series_with_frames_in_position_order() {
    // Review focus 1: a whole series in one file, its frames stored out of order.
    let frames: Vec<EnhancedFrame> = (0..24)
        .map(|i| EnhancedFrame::at([-100.0, -120.0, f64::from((i * 5) % 24) * 3.0]))
        .collect();
    let file = enhanced_series(&EnhancedOptions {
        common: SliceOptions::default(),
        frames,
    });
    let index = indexed(vec![("ENH/series.dcm".to_owned(), file)]);
    assert_eq!(index.series.len(), 1);
    let series = &index.series[0];
    assert_eq!(series.info.slices, 24);
    assert_eq!(series.info.problems, Vec::new());
    assert_eq!(series.info.spacing_mm, [0.9, 0.9, 3.0]);
    assert!(series.slices.iter().all(|s| s.entry == "ENH/series.dcm"));
    let stored_order: Vec<u32> = series
        .slices
        .iter()
        .map(|s| s.frame.expect("a frame number"))
        .collect();
    let by_position: Vec<u32> = (0..24)
        .map(|place| (0..24).find(|i| (i * 5) % 24 == place).expect("a frame"))
        .collect();
    assert_eq!(stored_order, by_position);
}

#[test]
fn index_gives_each_enhanced_file_its_own_series() {
    let make = |instance: i32| {
        let frames = (0..3)
            .map(|i| EnhancedFrame::at([0.0, 0.0, f64::from(i)]))
            .collect();
        let common = SliceOptions {
            instance_number: instance,
            ..SliceOptions::default()
        };
        enhanced_series(&EnhancedOptions { common, frames })
    };
    let index = indexed(vec![
        ("a.dcm".to_owned(), make(1)),
        ("b.dcm".to_owned(), make(2)),
    ]);
    assert_eq!(index.series.len(), 2);
    assert_ne!(index.series[0].info.id, index.series[1].info.id);
}

#[test]
fn index_skips_junk_and_says_which_files() {
    // Review focus 2: an export with a report, an index file and notes among the images.
    let mut files = stack(30, 5.0, &SliceOptions::default());
    files.push(("report.pdf".to_owned(), b"%PDF-1.7 not an image".to_vec()));
    files.push((
        "DICOMDIR".to_owned(),
        b"an index file, ignored without comment".to_vec(),
    ));
    files.push(("notes/readme.txt".to_owned(), b"hello".to_vec()));
    let index = indexed(files);
    assert_eq!(index.series.len(), 1);
    assert_eq!(index.series[0].info.slices, 30);
    assert_eq!(
        index.skipped,
        vec!["notes/readme.txt".to_owned(), "report.pdf".to_owned()]
    );
}

#[test]
fn index_reports_progress_and_ends_on_the_final_index() {
    let second = SliceOptions {
        series_uid: "1.2.3.4.2".to_owned(),
        series_number: 2,
        ..SliceOptions::default()
    };
    let mut files = stack(4, 5.0, &SliceOptions::default());
    files.extend(stack(4, 5.0, &second));
    let mut seen: Vec<Index> = Vec::new();
    let result = index(&mut MemorySource::new(files), &mut |progress| {
        seen.push(progress.clone());
    })
    .expect("indexing does not fail");
    assert!(seen.len() >= 2, "at least one report per series");
    assert_eq!(seen.last(), Some(&result));
    assert!(
        seen.iter().any(|i| i.series.len() == 1),
        "the first series was reported before the second arrived"
    );
}

#[test]
fn index_of_nothing_is_empty() {
    let index = indexed(Vec::new());
    assert_eq!(index, Index::default());
}

fn listed(
    number: i32,
    rows: u32,
    cols: u32,
    slices: usize,
    problems: Vec<SeriesProblem>,
) -> IndexedSeries {
    IndexedSeries {
        info: SeriesInfo {
            id: SeriesId(format!("1.2.{number}")),
            number,
            description: String::new(),
            modality: "MR".to_owned(),
            rows,
            cols,
            slices,
            spacing_mm: [1.0, 1.0, 1.0],
            voxel_to_patient: None,
            problems,
            ..SeriesInfo::default()
        },
        slices: Vec::new(),
    }
}

#[test]
fn best_series_is_the_one_with_the_most_voxels_among_clean_stacks() {
    let thick_scan = listed(1, 480, 400, 30, Vec::new());
    let fine_scan = listed(2, 480, 360, 72, Vec::new());
    let scout = listed(3, 256, 256, 5, Vec::new());
    let flagged = listed(4, 512, 512, 72, vec![SeriesProblem::UnevenSpacing]);
    let index = Index {
        series: vec![
            thick_scan,
            fine_scan.clone(),
            scout.clone(),
            flagged.clone(),
        ],
        ..Index::default()
    };
    assert_eq!(best_series(&index), Some(&fine_scan));
    let none_qualify = Index {
        series: vec![scout, flagged],
        ..Index::default()
    };
    assert_eq!(best_series(&none_qualify), None);
}

#[test]
fn best_series_breaks_a_tie_with_the_lower_series_number() {
    let index = Index {
        series: vec![
            listed(9, 256, 256, 20, Vec::new()),
            listed(4, 256, 256, 20, Vec::new()),
        ],
        ..Index::default()
    };
    assert_eq!(best_series(&index).map(|s| s.info.number), Some(4));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn index_never_fails_or_panics_on_arbitrary_bytes(
        files in proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..600), 0..6),
        prefix_with_magic in any::<bool>(),
    ) {
        let named: Vec<(String, Vec<u8>)> = files
            .into_iter()
            .enumerate()
            .map(|(i, mut bytes)| {
                if prefix_with_magic {
                    // Looks like a DICOM file for the first 132 bytes, then noise.
                    let mut start = vec![0u8; 128];
                    start.extend_from_slice(b"DICM");
                    start.append(&mut bytes);
                    bytes = start;
                }
                (format!("f{i}"), bytes)
            })
            .collect();
        let count = named.len();
        let result = index(&mut MemorySource::new(named), &mut |_| {});
        let index = result.expect("arbitrary bytes are skipped, not an error");
        prop_assert_eq!(index.skipped.len() + index.series.iter().map(|s| s.slices.len()).sum::<usize>(), count);
    }
}
