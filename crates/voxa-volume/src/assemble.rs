//! Stacking the slices of a series into a volume, and saying why a series cannot be one.

use voxa_core::{SeriesInfo, SeriesProblem, Volume};

/// Why a series cannot be shown in 3D. Each reads as a sentence for the person using Voxa.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum VolumeError {
    /// The series has fewer than two slices; the number is given.
    #[error("A 3D view needs at least two slices; this series has {0}.")]
    TooFewSlices(usize),
    /// The files do not place the slices in the patient.
    #[error("The files do not say where the slices are, so a 3D view cannot be built.")]
    NoGeometry,
    /// The series is not a regular stack; its own problems, which say why, are given.
    #[error("This series cannot be shown in 3D.")]
    Problems(Vec<SeriesProblem>),
    /// A slice is missing or has the wrong number of values; its place, counting from 0.
    #[error("Slice {} is not the size the series says, so a 3D view cannot be built.", .slice + 1)]
    SizeMismatch {
        /// Which slice, counting from 0.
        slice: usize,
    },
}

/// Whether a series is a regular stack that can be shown in 3D: no problems, a place in the
/// patient, and at least two slices.
///
/// # Errors
///
/// The reason it is not, problems first.
pub fn can_be_volume(info: &SeriesInfo) -> Result<(), VolumeError> {
    if !info.problems.is_empty() {
        return Err(VolumeError::Problems(info.problems.clone()));
    }
    if info.voxel_to_patient.is_none() {
        return Err(VolumeError::NoGeometry);
    }
    if info.slices < 2 {
        return Err(VolumeError::TooFewSlices(info.slices));
    }
    Ok(())
}

/// Stack the decoded slices of a series, given in the series' own order, into a volume placed
/// by the series' transform.
///
/// # Errors
///
/// As [`can_be_volume`], and [`VolumeError::SizeMismatch`] naming the first slice that is
/// missing or does not hold rows times columns values.
pub fn assemble(info: &SeriesInfo, slices: &[&[f32]]) -> Result<Volume, VolumeError> {
    can_be_volume(info)?;
    let to_patient = info.voxel_to_patient.ok_or(VolumeError::NoGeometry)?;
    let in_a_slice = info.rows as usize * info.cols as usize;
    let odd_one = slices.iter().position(|slice| slice.len() != in_a_slice);
    if let Some(slice) =
        odd_one.or((slices.len() != info.slices).then_some(slices.len().min(info.slices)))
    {
        return Err(VolumeError::SizeMismatch { slice });
    }
    let count = u32::try_from(slices.len()).map_err(|_| VolumeError::SizeMismatch { slice: 0 })?;
    let values = slices.concat();
    Volume::new([info.cols, info.rows, count], values, to_patient).ok_or(VolumeError::NoGeometry)
}

/// The smallest and largest values in a volume, leaving out any that are not finite numbers.
/// `None` when none is.
pub fn value_range(volume: &Volume) -> Option<(f32, f32)> {
    let finite = || {
        volume
            .values()
            .iter()
            .copied()
            .filter(|value| value.is_finite())
    };
    Some((finite().reduce(f32::min)?, finite().reduce(f32::max)?))
}

#[cfg(test)]
mod tests {
    use voxa_core::{Affine, SeriesId, SeriesProblem};

    use super::*;

    /// A series of 2 x 2 slices, half a millimetre across and 7.5 mm apart.
    fn series(slices: usize) -> SeriesInfo {
        let to_patient = Affine::from_columns(
            [0.5, 0.0, 0.0],
            [0.0, 0.5, 0.0],
            [0.0, 0.0, 7.5],
            [-100.0, -120.0, 10.0],
        );
        SeriesInfo {
            id: SeriesId("1.2.3".to_owned()),
            number: 1,
            description: String::new(),
            modality: "MR".to_owned(),
            rows: 2,
            cols: 2,
            slices,
            spacing_mm: [0.5, 0.5, 7.5],
            voxel_to_patient: Some(to_patient),
            problems: Vec::new(),
        }
    }

    /// Slices of four values each, counting up from 0 across the whole stack.
    fn counted(slices: u8) -> Vec<Vec<f32>> {
        (0..slices)
            .map(|slice| (0..4).map(|i| f32::from(slice * 4 + i)).collect())
            .collect()
    }

    fn stacked(info: &SeriesInfo, slices: &[Vec<f32>]) -> Result<Volume, VolumeError> {
        let borrowed: Vec<&[f32]> = slices.iter().map(Vec::as_slice).collect();
        assemble(info, &borrowed)
    }

    #[test]
    fn slices_are_stacked_in_order_with_the_series_own_placement() {
        let volume = stacked(&series(3), &counted(3)).expect("assembles");
        assert_eq!(volume.dims(), [2, 2, 3]);
        assert_eq!(
            volume.get(1, 0, 2),
            Some(9.0),
            "the second value of the third slice"
        );
        assert_eq!(Some(volume.voxel_to_patient()), series(3).voxel_to_patient);
    }

    #[test]
    fn a_series_that_cannot_be_a_volume_says_why() {
        assert_eq!(can_be_volume(&series(3)), Ok(()));
        assert_eq!(can_be_volume(&series(1)), Err(VolumeError::TooFewSlices(1)));
        let unplaced = SeriesInfo {
            voxel_to_patient: None,
            ..series(3)
        };
        assert_eq!(can_be_volume(&unplaced), Err(VolumeError::NoGeometry));
        let uneven = SeriesInfo {
            problems: vec![SeriesProblem::UnevenSpacing],
            ..series(3)
        };
        let expected = VolumeError::Problems(vec![SeriesProblem::UnevenSpacing]);
        assert_eq!(can_be_volume(&uneven), Err(expected.clone()));
        assert_eq!(
            stacked(&uneven, &counted(3)),
            Err(expected),
            "and is not assembled"
        );
    }

    #[test]
    fn a_slice_of_the_wrong_size_or_a_missing_slice_is_named() {
        let mut slices = counted(3);
        slices[1].pop();
        assert_eq!(
            stacked(&series(3), &slices),
            Err(VolumeError::SizeMismatch { slice: 1 })
        );
        let short = stacked(&series(3), &counted(2));
        assert_eq!(
            short,
            Err(VolumeError::SizeMismatch { slice: 2 }),
            "the third is not there"
        );
    }

    #[test]
    fn the_range_leaves_out_values_that_are_not_numbers() {
        let values = vec![1.0, f32::NAN, -3.0, f32::INFINITY];
        let volume = Volume::new([4, 1, 1], values, Affine::identity()).expect("adds up");
        assert_eq!(value_range(&volume), Some((-3.0, 1.0)));
        let empty = Volume::new([2, 1, 1], vec![f32::NAN; 2], Affine::identity()).expect("adds up");
        assert_eq!(value_range(&empty), None);
    }

    #[test]
    fn each_reason_reads_as_a_plain_sentence() {
        let said = |error: VolumeError| error.to_string();
        assert_eq!(
            said(VolumeError::TooFewSlices(1)),
            "A 3D view needs at least two slices; this series has 1."
        );
        assert_eq!(
            said(VolumeError::NoGeometry),
            "The files do not say where the slices are, so a 3D view cannot be built."
        );
        assert_eq!(
            said(VolumeError::Problems(Vec::new())),
            "This series cannot be shown in 3D."
        );
        assert_eq!(
            said(VolumeError::SizeMismatch { slice: 6 }),
            "Slice 7 is not the size the series says, so a 3D view cannot be built."
        );
    }
}
