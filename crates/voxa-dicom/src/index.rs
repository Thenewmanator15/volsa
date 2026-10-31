//! Reading the headers of a source and grouping its images into studies and series.

use std::collections::BTreeMap;

use voxa_core::{SeriesId, SeriesInfo, SeriesKind, SeriesProblem, StudyId, StudyInfo, VoxaError};

use crate::decode::unreadable;
use crate::geometry::{
    SAME_POSITION_MM, StackGeometry, is_image_plane, median, same_plane, stack_geometry,
};
use crate::header::{Header, Start, Told, read_header_from};
use crate::nesting::Refused;
use crate::source::{MAX_ENTRY_BYTES, SeriesSource};

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
    /// Names of the entries that are not DICOM images, in name order.
    pub skipped: Vec<String>,
    /// The entries that are not read, each with the reason, in name order.
    pub refused: Vec<Refused>,
}

/// Read the header of every entry in `source` and group the images into studies and series.
///
/// Only headers are read, never pixels: of each file, no more than its header needs (see
/// [`SeriesSource::visit_files`]). Entries that are not DICOM images are listed in
/// [`Index::skipped`], and files refused unread, with why, in [`Index::refused`]; a `DICOMDIR`
/// is passed over without comment, and without being read. `on_progress` is given the index so far each time a new series appears, at intervals in
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
    source.visit_files(&mut |name, contents| {
        let at = seen;
        seen += 1;
        if is_directory_file(name) {
            return Ok(());
        }
        let header = match read_header_from(contents, MAX_ENTRY_BYTES)? {
            Start::Image(header) => *header,
            Start::NotAnImage => {
                found.skipped.push(name.to_owned());
                return Ok(());
            }
            Start::Refused(reason) => {
                found.refused.push(Refused {
                    entry: name.to_owned(),
                    reason: reason.to_owned(),
                });
                return Ok(());
            }
        };
        let starts_a_series = found.add(name.to_owned(), header);
        if starts_a_series || at % REPORT_EVERY == 0 {
            on_progress(&found.index());
        }
        Ok(())
    })?;
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
}

impl Found {
    /// Add an image. True when it is the first of its series.
    fn add(&mut self, entry: String, header: Header) -> bool {
        let images = self.series.entry(series_key(&entry, &header)).or_default();
        images.push(Image { entry, header });
        images.len() == 1
    }

    /// The index of everything added so far.
    fn index(&self) -> Index {
        let mut by_study: BTreeMap<&str, (StudyInfo, Vec<IndexedSeries>)> = BTreeMap::new();
        for (id, images) in &self.series {
            let Some(first) = images.first().map(|image| &image.header) else {
                continue;
            };
            let (_, listed) = by_study
                .entry(first.study_uid.as_str())
                .or_insert_with(|| (study_info(first), Vec::new()));
            listed.extend(volumes::listed(id, images));
        }
        let mut studies: Vec<_> = by_study.into_values().collect();
        studies.sort_by(|(a, _), (b, _)| {
            (a.date.is_none(), &a.date, &a.id).cmp(&(b.date.is_none(), &b.date, &b.id))
        });
        let mut index = Index {
            skipped: self.skipped.clone(),
            refused: self.refused.clone(),
            ..Index::default()
        };
        index.skipped.sort();
        index.refused.sort_by(|a, b| a.entry.cmp(&b.entry));
        for (mut study, mut listed) in studies {
            listed.sort_by(|a, b| in_the_list(&a.info).cmp(&in_the_list(&b.info)));
            study.series = listed.iter().map(|series| series.info.clone()).collect();
            index.studies.push(study);
            index.series.append(&mut listed);
        }
        index
    }
}

/// Where a series comes in its study's list: by its number, then its identifier, with the
/// volumes of a series that holds several kept together and in their own order.
fn in_the_list(info: &SeriesInfo) -> (i32, &SeriesId, usize) {
    match &info.set {
        Some(set) => (info.number, &set.of, set.at),
        None => (info.number, &info.id, 0),
    }
}

/// Single-frame files share their series; a file holding many frames is a series by itself.
fn series_key(entry: &str, header: &Header) -> String {
    if !header.multi_frame {
        return header.series_uid.clone();
    }
    let own = if header.sop_uid.is_empty() {
        entry
    } else {
        header.sop_uid.as_str()
    };
    format!("{}/{own}", header.series_uid)
}

fn study_info(header: &Header) -> StudyInfo {
    StudyInfo {
        id: StudyId(header.study_uid.clone()),
        date: header.study_date.clone(),
        description: header.study_description.clone(),
        patient_name: header.patient_name.clone(),
        patient_id: header.patient_id.clone(),
        series: Vec::new(),
    }
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
    let between_slices = step
        .or(first.slice_thickness.filter(|mm| *mm > 0.0))
        .unwrap_or(1.0);
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
            voxel_to_patient,
            problems,
            scan,
            colour: first.colour,
            ..SeriesInfo::default()
        },
        slices: slices
            .iter()
            .map(|slice| SliceRef {
                entry: slice.entry.to_owned(),
                frame: slice.frame,
            })
            .collect(),
    }
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
mod tests;
#[cfg(test)]
mod volumes_tests;
