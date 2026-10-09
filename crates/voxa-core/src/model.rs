//! What a scan export contains: studies, the series inside them, and anything wrong with a series.

use crate::space::{Affine, Patient, Voxel};

/// Identifier of a series: its DICOM `SeriesInstanceUID`, followed by the file's own identifier
/// when one file holds the whole series.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SeriesId(pub String);

/// Identifier of a study (its DICOM `StudyInstanceUID`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StudyId(pub String);

/// A reason a series cannot be treated as a regular volume. Each reads as a sentence for the
/// person using Voxa.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SeriesProblem {
    /// The series is not magnetic resonance; the modality found is given.
    #[error("This is not an MRI series ({0}).")]
    NotMr(String),
    /// Too few slices for a volume; the count is given.
    #[error("There are only {0} slices, too few to treat as a volume.")]
    TooFewSlices(usize),
    /// The slices do not all have the same number of rows and columns.
    #[error("The slices are not all the same size, so this is shown as a plain stack.")]
    MixedSizes,
    /// The gaps between neighbouring slices differ.
    #[error("The slices are not evenly spaced, so this is shown as a plain stack.")]
    UnevenSpacing,
    /// Several slices share one position, as in diffusion or functional series.
    #[error(
        "Several slices share the same position (a series of repeated volumes), so this is shown as a plain stack."
    )]
    RepeatedPositions,
    /// The slices lie in different planes, as in a set of locating images.
    #[error("The slices do not all face the same way, so this is shown as a plain stack.")]
    MixedOrientations,
    /// Position, orientation or pixel spacing is absent.
    #[error("The files do not say where the slices are, so orientation and scale are unavailable.")]
    MissingGeometry,
    /// The pixel data uses a compression Voxa cannot read; its name is given.
    #[error("The images are compressed in a way Voxa cannot read yet ({0}).")]
    UnsupportedTransferSyntax(String),
}

/// One series of images.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesInfo {
    /// The series identifier.
    pub id: SeriesId,
    /// The series number given by the scanner.
    pub number: i32,
    /// The scanner's description of the series.
    pub description: String,
    /// The DICOM modality, such as `MR`.
    pub modality: String,
    /// Rows in each slice.
    pub rows: u32,
    /// Columns in each slice.
    pub cols: u32,
    /// Number of slices.
    pub slices: usize,
    /// Distance between voxel centres along columns, rows and slices, in millimetres.
    pub spacing_mm: [f64; 3],
    /// Where each voxel lies in the patient, when the files say.
    pub voxel_to_patient: Option<Affine<Voxel, Patient>>,
    /// Anything that stops the series being treated as a regular volume.
    pub problems: Vec<SeriesProblem>,
}

impl SeriesInfo {
    /// Whether the series knows where its voxels are in the patient.
    pub fn has_geometry(&self) -> bool {
        self.voxel_to_patient.is_some()
    }

    /// Distance between neighbouring slices, in millimetres.
    pub fn slice_thickness_mm(&self) -> f64 {
        let [_, _, between_slices] = self.spacing_mm;
        between_slices
    }

    /// Total number of voxels: rows × columns × slices.
    pub fn voxel_count(&self) -> u64 {
        u64::from(self.rows) * u64::from(self.cols) * self.slices as u64
    }
}

/// One visit to the scanner: a set of series.
#[derive(Clone, PartialEq)]
pub struct StudyInfo {
    /// The study identifier.
    pub id: StudyId,
    /// The study date as written in the files (`YYYYMMDD`), when present.
    pub date: Option<String>,
    /// The scanner's description of the study.
    pub description: String,
    /// The patient's name as written in the files. Hidden in the interface unless asked for,
    /// and never logged.
    pub patient_name: Option<String>,
    /// The patient's identifier as written in the files. Hidden in the interface unless asked
    /// for, and never logged.
    pub patient_id: Option<String>,
    /// The series in this study, ordered by series number.
    pub series: Vec<SeriesInfo>,
}

impl std::fmt::Debug for StudyInfo {
    /// Prints everything but the patient's name and identifier, of which it says only whether
    /// they are there. A study can then appear in a log or a failed test, alone or inside
    /// something larger, without naming anyone.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hidden = |field: &Option<String>| field.as_ref().map(|_| "<hidden>");
        f.debug_struct("StudyInfo")
            .field("id", &self.id)
            .field("date", &self.date)
            .field("description", &self.description)
            .field("patient_name", &hidden(&self.patient_name))
            .field("patient_id", &hidden(&self.patient_id))
            .field("series", &self.series)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::Affine;

    fn series(geometry: bool) -> SeriesInfo {
        SeriesInfo {
            id: SeriesId("1.2.3".to_owned()),
            number: 4,
            description: "made-up axial".to_owned(),
            modality: "MR".to_owned(),
            rows: 480,
            cols: 400,
            slices: 30,
            spacing_mm: [0.7, 0.7, 7.5],
            voxel_to_patient: geometry.then(Affine::identity),
            problems: Vec::new(),
        }
    }

    #[test]
    fn geometry_is_reported_only_when_present() {
        assert!(series(true).has_geometry());
        assert!(!series(false).has_geometry());
    }

    #[test]
    fn slice_thickness_is_the_third_spacing() {
        assert_eq!(series(true).slice_thickness_mm(), 7.5);
    }

    #[test]
    fn voxel_count_multiplies_the_three_sizes() {
        assert_eq!(series(true).voxel_count(), 480 * 400 * 30);
    }

    #[test]
    fn problems_read_as_plain_sentences() {
        assert_eq!(
            SeriesProblem::UnevenSpacing.to_string(),
            "The slices are not evenly spaced, so this is shown as a plain stack."
        );
        assert_eq!(
            SeriesProblem::UnsupportedTransferSyntax("JPEG XL".to_owned()).to_string(),
            "The images are compressed in a way Voxa cannot read yet (JPEG XL)."
        );
        assert_eq!(
            SeriesProblem::MissingGeometry.to_string(),
            "The files do not say where the slices are, so orientation and scale are unavailable."
        );
        assert_eq!(
            SeriesProblem::MixedOrientations.to_string(),
            "The slices do not all face the same way, so this is shown as a plain stack."
        );
    }

    #[test]
    fn printing_a_study_does_not_name_the_patient() {
        let study = StudyInfo {
            id: StudyId("1.2".to_owned()),
            date: Some("20210830".to_owned()),
            description: "Head".to_owned(),
            patient_name: Some("Phantom^Test".to_owned()),
            patient_id: Some("PHANTOM-001".to_owned()),
            series: Vec::new(),
        };
        let printed = format!("{study:?}");
        assert!(
            !printed.contains("Phantom") && !printed.contains("PHANTOM-001"),
            "{printed}"
        );
        assert!(
            printed.contains("20210830") && printed.contains("<hidden>"),
            "{printed}"
        );
        let nameless = StudyInfo {
            patient_name: None,
            ..study
        };
        assert!(format!("{nameless:?}").contains("patient_name: None"));
    }

    #[test]
    fn ids_order_and_compare_by_their_text() {
        assert!(StudyId("1.2".to_owned()) < StudyId("1.3".to_owned()));
        assert_eq!(SeriesId("a".to_owned()), SeriesId("a".to_owned()));
    }
}
