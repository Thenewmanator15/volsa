//! A small, rough volume of a long series, made from a few of its slices while the rest are
//! still being decoded, so that the 3D view has something to show at once.

use volsa_core::{Affine, Patient, Volume, Voxel};

/// A preview of a series of `dims`, placed by `voxel_to_patient`, from `slices`: each its
/// number in the series and its values, the slices evenly spaced and in order. Each is
/// reduced to `ceil(cols / across)` by `ceil(rows / across)` by the means of blocks of
/// `across` voxels a side (fewer at the far edges), and voxel `(i, j, k)` of the preview lies
/// at the middle of full voxels `across × i` to `across × i + across − 1`, likewise along the
/// rows, and at slice `slices[k].0`. `None` with no slices, `across` of 0, or a slice that is
/// not the size `dims` says.
pub fn preview(
    voxel_to_patient: Affine<Voxel, Patient>,
    dims: [u32; 3],
    slices: &[(u32, Vec<f32>)],
    across: u32,
) -> Option<Volume> {
    let [cols, rows, _] = dims;
    if across == 0
        || slices
            .iter()
            .any(|(_, slice)| slice.len() != cols as usize * rows as usize)
    {
        return None;
    }
    let reduced: Vec<(u32, Vec<f32>)> = slices
        .iter()
        .map(|(at, slice)| (*at, reduced(slice, [cols, rows], across)))
        .collect();
    preview_of_reduced(voxel_to_patient, dims, &reduced, across)
}

/// As [`preview`], from slices each already reduced by [`reduced`]: so that a preview of a
/// long series is made without holding its slices in full. `None` with no slices, `across`
/// of 0, or a slice that is not the size a reduced one is.
pub fn preview_of_reduced(
    voxel_to_patient: Affine<Voxel, Patient>,
    dims: [u32; 3],
    slices: &[(u32, Vec<f32>)],
    across: u32,
) -> Option<Volume> {
    let [cols, rows, _] = dims;
    let (first, _) = slices.first()?;
    let (last, _) = slices.last()?;
    if across == 0 {
        return None;
    }
    let small = [cols.div_ceil(across), rows.div_ceil(across)];
    let count = u32::try_from(slices.len()).ok()?;
    let mut values = Vec::with_capacity(small[0] as usize * small[1] as usize * slices.len());
    for (_, slice) in slices {
        if slice.len() != small[0] as usize * small[1] as usize {
            return None;
        }
        values.extend_from_slice(slice);
    }
    let apart = if count > 1 {
        f64::from(last - first) / f64::from(count - 1)
    } else {
        1.0
    };
    let step = f64::from(across);
    let middle = (step - 1.0) / 2.0;
    let onto_full: Affine<Voxel, Voxel> = Affine::from_columns(
        [step, 0.0, 0.0],
        [0.0, step, 0.0],
        [0.0, 0.0, apart],
        [middle, middle, f64::from(*first)],
    );
    let to_patient = onto_full.then(&voxel_to_patient);
    Volume::new([small[0], small[1], count], values, to_patient)
}

/// One slice of `cols` by `rows` reduced to the means of its blocks of `across` a side: the
/// slice as a preview holds it.
pub fn reduced(slice: &[f32], [cols, rows]: [u32; 2], across: u32) -> Vec<f32> {
    let [cols, rows, across] = [cols, rows, across].map(|count| count as usize);
    let (wide, high) = (cols.div_ceil(across), rows.div_ceil(across));
    let mut sums = vec![(0.0_f64, 0_u32); wide * high];
    for (row, line) in slice.chunks(cols).take(rows).enumerate() {
        let start = row / across * wide;
        for (col, value) in line.iter().enumerate() {
            if let Some((sum, count)) = sums.get_mut(start + col / across) {
                *sum += f64::from(*value);
                *count += 1;
            }
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a mean of 32-bit values is one again"
    )]
    let mean = |(sum, count): (f64, u32)| (sum / f64::from(count.max(1))) as f32;
    sums.into_iter().map(mean).collect()
}

#[cfg(test)]
mod tests {
    use volsa_core::{Grid, Point};

    use super::*;

    /// A series of 10 x 6 voxels of 0.5 by 0.5 by 2 mm, tilted a little, of `slices` slices.
    fn placed() -> Affine<Voxel, Patient> {
        Affine::from_columns(
            [0.5, 0.0, 0.05],
            [0.0, 0.5, 0.0],
            [0.0, 0.2, 2.0],
            [-10.0, 4.0, 30.0],
        )
    }

    /// Slice `n` of a ramp that rises by 1 a column and by 100 a slice.
    fn ramp(n: u32) -> Vec<f32> {
        (0..6)
            .flat_map(|_| {
                (0..10).map(move |col| f32::from(u16::try_from(col + 100 * n).expect("small")))
            })
            .collect()
    }

    #[test]
    fn a_ramp_is_reduced_to_its_block_means() {
        let slices: Vec<(u32, Vec<f32>)> = [0, 8, 16].into_iter().map(|n| (n, ramp(n))).collect();
        let small = preview(placed(), [10, 6, 20], &slices, 4).expect("a preview");
        assert_eq!(small.dims(), [3, 2, 3]);
        // Columns 0-3, 4-7 and the last two, 8 and 9.
        let first_row: Vec<f32> = (0..3)
            .map(|col| small.value(col, 0, 0).expect("in"))
            .collect();
        assert_eq!(first_row, [1.5, 5.5, 8.5]);
        assert_eq!(
            small.value(2, 1, 2),
            Some(1608.5),
            "slice 16, the last rows and columns"
        );
    }

    #[test]
    fn the_preview_lies_where_the_series_lies() {
        let slices: Vec<(u32, Vec<f32>)> = [0, 8, 16].into_iter().map(|n| (n, ramp(n))).collect();
        let small = preview(placed(), [10, 6, 17], &slices, 4).expect("a preview");
        let at = |volume: &Affine<Voxel, Patient>, [x, y, z]: [f64; 3]| {
            volume.apply(Point::new(x, y, z))
        };
        let full = placed();
        // Preview voxel (1, 0, 2) is the middle of full columns 4-7 and rows 0-3, at slice 16.
        let here = at(&small.voxel_to_patient(), [1.0, 0.0, 2.0]);
        let there = at(&full, [5.5, 1.5, 16.0]);
        let apart = (here.x - there.x)
            .hypot(here.y - there.y)
            .hypot(here.z - there.z);
        assert!(apart < 1e-9, "{apart} mm apart");
        // The outer corners are within one of the preview's own voxels of the series': it
        // stands for blocks of 4 x 4 voxels and 8 slices.
        let series = Volume::new([10, 6, 17], vec![0.0; 1020], full).expect("a volume");
        let preview_voxel = 2.0_f64.hypot(2.0).hypot(16.0);
        for (one, other) in small.corners().iter().zip(series.corners()) {
            let apart = (one.x - other.x)
                .hypot(one.y - other.y)
                .hypot(one.z - other.z);
            assert!(apart <= preview_voxel, "{apart} mm apart");
        }
    }

    #[test]
    fn nothing_to_preview_or_a_slice_of_the_wrong_size_makes_none() {
        assert!(preview(placed(), [10, 6, 20], &[], 4).is_none());
        assert!(preview(placed(), [10, 6, 20], &[(0, vec![0.0; 59])], 4).is_none());
        assert!(preview(placed(), [10, 6, 20], &[(0, ramp(0))], 0).is_none());
    }
}
