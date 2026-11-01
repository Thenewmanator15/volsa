//! Reading the headers of a source and grouping its images into studies and series.

use std::collections::BTreeMap;

use voxa_core::{SeriesId, SeriesInfo, SeriesKind, SeriesProblem, StudyInfo, VoxaError};

use crate::decode::unreadable;
use crate::document::IndexedDocument;
use crate::geometry::{
    SAME_POSITION_MM, StackGeometry, is_image_plane, median, same_plane, stack_geometry,
};
use crate::header::{Header, Told};
use crate::nesting::Refused;
use crate::source::SeriesSource;

mod found;
mod threads;
mod volumes;

/// A gap further than this fraction from the typical gap makes a stack uneven.
const UNEVEN_FRACTION: f64 = 0.1;
/// Between new series, progress is reported once in this many entries.
const REPORT_EVERY: usize = 32;
/// The fewest slices a series needs to be the one opened automatically.
const MIN_SLICES_FOR_BEST: usize = 10;

/// Where to find the pixels of one slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceRef {
    /// Name of the entry in the source that holds the slice.
    pub entry: String,
    /// For a file holding many frames, which one, counting from 0 in the order they are
    /// stored; `None` for a file holding a single slice.
    pub frame: Option<u32>,
}

/// A series and its slices, in order along the slice normal.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedSeries {
    /// What the series is.
    pub info: SeriesInfo,
    /// Its slices, in the order they are shown.
    pub slices: Vec<SliceRef>,
}

/// Everything found in a source.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Index {
    /// Studies by date, then identifier; undated studies come last.
    pub studies: Vec<StudyInfo>,
    /// Every series, study by study in the order of `studies`, then by series number.
    pub series: Vec<IndexedSeries>,
    /// Names of the entries that are neither DICOM images nor documents, in name order.
    pub skipped: Vec<String>,
    /// The entries that are not read, each with the reason, in name order.
    pub refused: Vec<Refused>,
    /// The DICOM files that hold no image but a document: reports, PDFs and the like, in study
    /// order, then by entry name.
    pub documents: Vec<IndexedDocument>,
}

/// Read the header of every entry in `source` and group the images into studies and series.
///
/// Only headers are read, never pixels: of each file, no more than its header needs (see
/// [`SeriesSource::visit_files`]). A DICOM file with no image is listed in
/// [`Index::documents`]; any other entry that is not a DICOM image is listed in
/// [`Index::skipped`], and a file refused unread, with why, in [`Index::refused`]. A `DICOMDIR`
/// is passed over without comment, and without being read.
/// `on_progress` is given the index so far each time a new series appears, at intervals in
/// between, and once more with the finished index.
///
/// # Errors
///
/// Returns the source's own error when an entry cannot be read from it.
pub fn index(
    source: &mut dyn SeriesSource,
    on_progress: &mut dyn FnMut(&Index),
) -> Result<Index, VoxaError> {
    let mut found = Found::default();
    let mut seen = 0;
    if let Some(runs) = threads::read_on_threads(source) {
        // Read on several threads; taken in here in name order, as if read in turn.
        for (read, error) in runs {
            for (name, read) in read {
                if found.take(name, read, seen) {
                    on_progress(&found.index());
                }
                seen += 1;
            }
            if let Some(error) = error {
                return Err(error);
            }
        }
    } else {
        source.visit_files(&mut |name, contents| {
            let read = threads::read_one(name, contents)?;
            if found.take(name.to_owned(), read, seen) {
                on_progress(&found.index());
            }
            seen += 1;
            Ok(())
        })?;
    }
    let finished = found.index();
    on_progress(&finished);
    Ok(finished)
}

/// The series to open first: the one with the most voxels among the volumes of at least
/// ten slices. A tie goes to the lower series number, and then to the earlier
/// volume of a series that holds several. `None` when none qualifies.
pub fn best_series(index: &Index) -> Option<&IndexedSeries> {
    index
        .series
        .iter()
        .filter(|series| {
            series.info.slices >= MIN_SLICES_FOR_BEST && series.info.kind() == SeriesKind::Volume
        })
        .max_by(|a, b| {
            // Of the volumes of one series, which are all alike, the first.
            let place = |series: &IndexedSeries| series.info.set.as_ref().map_or(0, |set| set.at);
            let by_size = a.info.voxel_count().cmp(&b.info.voxel_count());
            by_size
                .then(b.info.number.cmp(&a.info.number))
                .then(place(b).cmp(&place(a)))
        })
}

/// Whether an entry is a `DICOMDIR`: a list of the other files, with no images of its own.
fn is_directory_file(name: &str) -> bool {
    name.rsplit(['/', '\\'])
        .next()
        .is_some_and(|last| last.eq_ignore_ascii_case("DICOMDIR"))
}

/// One image file and what its header said.
struct Image {
    entry: String,
    header: Header,
}

/// What has been read so far.
#[derive(Default)]
struct Found {
    /// Images by series identifier, each list in the order the images arrived.
    series: BTreeMap<String, Vec<Image>>,
    skipped: Vec<String>,
    refused: Vec<Refused>,
    documents: Vec<IndexedDocument>,
}

/// One slice of a series, before the slices are put in order.
#[derive(Clone, Copy)]
struct Slice<'a> {
    entry: &'a str,
    frame: Option<u32>,
    position: Option<[f64; 3]>,
    instance: i32,
    size: (u32, u32),
    /// What the image says that sets it apart from another at its position.
    told: Told,
    /// The header of the file it is in.
    header: &'a Header,
}

fn slices_of(image: &Image) -> impl Iterator<Item = Slice<'_>> {
    let header = &image.header;
    header
        .positions
        .iter()
        .enumerate()
        .map(move |(number, position)| Slice {
            entry: &image.entry,
            frame: if header.multi_frame {
                u32::try_from(number).ok()
            } else {
                None
            },
            position: *position,
            instance: header.instance_number,
            size: (header.rows, header.cols),
            told: header.told.get(number).copied().unwrap_or_default(),
            header,
        })
}

/// What the files say of how a series was acquired, from the header it is described from.
/// Images in several planes are in none of them as a series.
fn facts(first: &Header, problems: &[SeriesProblem]) -> voxa_core::ScanFacts {
    let mut scan = first.scan.clone();
    if problems.contains(&SeriesProblem::MixedOrientations) {
        scan.plane = None;
    }
    scan
}

/// Put slices in order and say what is known about the stack they make. `first` is the header
/// the series is described from, and `images` are all the files it was found in.
fn assembled(
    id: String,
    first: &Header,
    images: &[Image],
    mut slices: Vec<Slice<'_>>,
) -> IndexedSeries {
    let mut problems = Vec::new();
    let mut step = None;
    let mut voxel_to_patient = None;
    match placement(first, images, &slices) {
        Ok(stack) => {
            slices = stack
                .order
                .iter()
                .filter_map(|at| slices.get(*at).copied())
                .collect();
            step = typical_step(&stack.steps_mm, &mut problems);
            voxel_to_patient = Some(stack.voxel_to_patient);
        }
        Err(problem) => {
            slices.sort_by_key(|slice| (slice.instance, slice.entry, slice.frame));
            problems.push(problem);
        }
    }
    if slices
        .iter()
        .any(|slice| slice.size != (first.rows, first.cols))
    {
        // Without one grid for every slice there is no single transform to the patient.
        problems.push(SeriesProblem::MixedSizes);
        voxel_to_patient = None;
    }
    let unreadable = images
        .iter()
        .find_map(|image| unreadable(&image.header.transfer_syntax));
    problems.extend(unreadable.map(SeriesProblem::UnsupportedTransferSyntax));
    let [between_rows, between_cols] = first.pixel_spacing.unwrap_or([1.0, 1.0]);
    let slice_step = step.or(first.slice_thickness.filter(|mm| *mm > 0.0));
    let between_slices = slice_step.unwrap_or(1.0);
    let scan = facts(first, &problems);
    IndexedSeries {
        info: SeriesInfo {
            id: SeriesId(id),
            number: first.series_number,
            description: first.series_description.clone(),
            modality: first.modality.clone(),
            rows: first.rows,
            cols: first.cols,
            slices: slices.len(),
            spacing_mm: [between_cols, between_rows, between_slices],
            slice_step_given: slice_step.is_some(),
            spacing_given: first.pixel_spacing.is_some(),
            voxel_to_patient,
            problems,
            scan,
            colour: first.colour,
            rescale_type: first.rescale_type.clone(),
            inverted: first.inverted,
            ..SeriesInfo::default()
        },
        slices: slice_refs(&slices),
    }
}

/// Where to find each of `slices`, in their order.
fn slice_refs(slices: &[Slice<'_>]) -> Vec<SliceRef> {
    slices
        .iter()
        .map(|slice| SliceRef {
            entry: slice.entry.to_owned(),
            frame: slice.frame,
        })
        .collect()
}

/// Where the stack lies in the patient, or why that cannot be said.
fn placement(
    first: &Header,
    images: &[Image],
    slices: &[Slice<'_>],
) -> Result<StackGeometry, SeriesProblem> {
    let positions: Option<Vec<[f64; 3]>> = slices.iter().map(|slice| slice.position).collect();
    let complete = images
        .iter()
        .all(|image| image.header.orientation.is_some() && image.header.pixel_spacing.is_some());
    let (Some(positions), Some(orientation), Some(spacing), true) =
        (positions, first.orientation, first.pixel_spacing, complete)
    else {
        return Err(SeriesProblem::MissingGeometry);
    };
    if !is_image_plane(orientation) {
        return Err(SeriesProblem::MissingGeometry);
    }
    let mut directions = images.iter().filter_map(|image| image.header.orientation);
    if directions.any(|other| !same_plane(other, orientation)) {
        return Err(SeriesProblem::MixedOrientations);
    }
    stack_geometry(&positions, orientation, spacing).ok_or(SeriesProblem::MissingGeometry)
}

/// The typical gap between neighbouring slices, noting anything irregular about the gaps.
///
/// Slices that share a position are a problem of their own, so evenness is judged among the
/// gaps between slices that do not.
fn typical_step(steps_mm: &[f64], problems: &mut Vec<SeriesProblem>) -> Option<f64> {
    let apart: Vec<f64> = steps_mm
        .iter()
        .copied()
        .filter(|gap| *gap >= SAME_POSITION_MM)
        .collect();
    let typical = (!apart.is_empty()).then(|| median(&apart));
    let uneven = typical.is_some_and(|step| {
        apart
            .iter()
            .any(|gap| (gap - step).abs() > UNEVEN_FRACTION * step)
    });
    if uneven {
        problems.push(SeriesProblem::UnevenSpacing);
    }
    if apart.len() < steps_mm.len() {
        problems.push(SeriesProblem::RepeatedPositions);
    }
    typical
}

#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod reading_tests;
#[cfg(test)]
mod scan_tests;
#[cfg(test)]
mod spacing_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod threads_tests;
#[cfg(test)]
mod volumes_tests;
