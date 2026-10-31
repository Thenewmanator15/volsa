//! Making a volume smaller: to fit a graphics card that cannot hold it whole, and to know
//! which parts of it are empty.

use voxa_core::{Affine, Grid, HeldVolume, SlabRaw, SliceIn, Stored, Volume, Voxel};

/// How many bytes a voxel takes on the graphics card.
const BYTES_PER_VOXEL: u64 = 2;

/// A volume with half as many voxels each way, in the same place in the patient. Each new
/// voxel is the mean of the block of 2 x 2 x 2 it replaces; where a side has an odd number of
/// voxels, the last block is the mean of the voxels that are there.
pub fn halved(volume: &Volume) -> Volume {
    let dims = volume.dims().map(|count| count.div_ceil(2));
    let [cols, rows, slices] = dims;
    let mut values = Vec::with_capacity(cols as usize * rows as usize * slices as usize);
    for slice in 0..slices {
        for row in 0..rows {
            for col in 0..cols {
                values.push(block_mean(volume, col, row, slice));
            }
        }
    }
    // The centre of a new voxel is the middle of its block: half an old voxel past the first.
    let onto_old: Affine<Voxel, Voxel> =
        Affine::from_columns([2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0], [0.5; 3]);
    let to_patient = onto_old.then(&volume.voxel_to_patient());
    Volume::new(dims, values, to_patient).unwrap_or_else(|| volume.clone())
}

/// The mean of the old voxels that the new voxel at this column, row and slice replaces.
fn block_mean(volume: &Volume, col: u32, row: u32, slice: u32) -> f32 {
    let (mut sum, mut count) = (0.0_f32, 0_u8);
    for along_slices in 0..2 {
        for along_rows in 0..2 {
            for along_cols in 0..2 {
                let old = (
                    2 * col + along_cols,
                    2 * row + along_rows,
                    2 * slice + along_slices,
                );
                if let Some(value) = volume.get(old.0, old.1, old.2) {
                    sum += value;
                    count += 1;
                }
            }
        }
    }
    sum / f32::from(count.max(1))
}

/// The most bytes in one slab of a halved held volume.
const HALVED_SLAB_BYTES: usize = 32 << 20;

/// A held volume with half as many voxels each way, in the same place in the patient, held
/// the same way with the same rescale: each new voxel is the mean of the block it replaces,
/// as [`halved`] makes it, rounded to the nearest whole number where the numbers are whole.
/// For a graphics card that cannot hold the volume whole; what is measured is always read
/// from the volume itself. `None` if the volume cannot be held halved.
pub fn halved_held(held: &HeldVolume) -> Option<HeldVolume> {
    let dims = held.dims().map(|count| count.div_ceil(2));
    let onto_old: Affine<Voxel, Voxel> =
        Affine::from_columns([2.0, 0.0, 0.0], [0.0, 2.0, 0.0], [0.0, 0.0, 2.0], [0.5; 3]);
    let to_patient = onto_old.then(&held.voxel_to_patient());
    let (stored, rescale) = (held.stored(), held.rescale());
    let half = HeldVolume::new(dims, to_patient, stored, rescale, HALVED_SLAB_BYTES)?;
    for slice in 0..dims[2] {
        let means = slice_means(held, slice)?;
        put_means(&half, slice, &means)?;
    }
    Some(half)
}

/// The means of the blocks that make new slice `slice` of a halved held volume, as numbers.
fn slice_means(held: &HeldVolume, slice: u32) -> Option<Vec<f64>> {
    let [cols, rows, slices] = held.dims();
    let read = |at: u32| {
        held.with_slice(at, |raw| -> Vec<f64> {
            match raw {
                SlabRaw::Unsigned(numbers) => numbers.iter().map(|n| f64::from(*n)).collect(),
                SlabRaw::Signed(numbers) => numbers.iter().map(|n| f64::from(*n)).collect(),
                SlabRaw::Float(values) => values.iter().map(|v| f64::from(*v)).collect(),
            }
        })
    };
    let old: Vec<Vec<f64>> = [2 * slice, 2 * slice + 1]
        .into_iter()
        .filter(|at| *at < slices)
        .map(read)
        .collect::<Option<_>>()?;
    let [new_cols, new_rows] = [cols.div_ceil(2), rows.div_ceil(2)];
    let mut means = Vec::with_capacity(new_cols as usize * new_rows as usize);
    for row in 0..new_rows {
        for col in 0..new_cols {
            let (mut sum, mut count) = (0.0, 0.0);
            for numbers in &old {
                for (c, r) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (c, r) = (2 * col + c, 2 * row + r);
                    if c < cols && r < rows {
                        sum += numbers.get((r * cols + c) as usize).copied().unwrap_or(0.0);
                        count += 1.0;
                    }
                }
            }
            means.push(if count > 0.0 { sum / count } else { 0.0 });
        }
    }
    Some(means)
}

/// Put a halved slice of means in, as the volume holds its numbers.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a mean of numbers that fit lies between them, and fits too"
)]
fn put_means(half: &HeldVolume, slice: u32, means: &[f64]) -> Option<()> {
    let put = match half.stored() {
        Stored::Unsigned16 => {
            let numbers: Vec<u16> = means.iter().map(|m| m.round() as u16).collect();
            half.put_slice(slice, SliceIn::Unsigned(&numbers))
        }
        Stored::Signed16 => {
            let numbers: Vec<i16> = means.iter().map(|m| m.round() as i16).collect();
            half.put_slice(slice, SliceIn::Signed(&numbers))
        }
        Stored::Float32 => {
            let values: Vec<f32> = means.iter().map(|m| *m as f32).collect();
            half.put_slice(slice, SliceIn::Float(&values))
        }
    };
    put.ok()
}

/// How many times a volume of `dims` must be halved before no side is longer than `max_side`
/// and it takes no more than `max_bytes` on the graphics card, at two bytes a voxel. A volume
/// of one voxel is never halved, whatever the limits.
pub fn halvings_needed(dims: [u32; 3], max_side: u32, max_bytes: u64) -> u32 {
    let fits = |dims: [u32; 3]| {
        let bytes = dims
            .into_iter()
            .map(u64::from)
            .try_fold(BYTES_PER_VOXEL, u64::checked_mul);
        dims.into_iter().all(|side| side <= max_side) && bytes.is_some_and(|b| b <= max_bytes)
    };
    let mut dims = dims;
    let mut halvings = 0;
    while dims.into_iter().any(|side| side > 1) && !fits(dims) {
        dims = dims.map(|side| side.div_ceil(2));
        halvings += 1;
    }
    halvings
}

/// The largest value in each block of `block` voxels a side, and how many blocks there are
/// each way: a coarse copy that shows which parts of a volume hold nothing bright enough to
/// draw. Blocks are stored as voxels are, columns fastest. Values that are not numbers are
/// left out; a block of nothing else holds negative infinity. A `block` of 0 is taken as 1.
pub fn block_maxima(volume: &Volume, block: u32) -> (Vec<f32>, [u32; 3]) {
    let block = block.max(1);
    let dims = volume.dims().map(|count| count.div_ceil(block));
    let [cols, rows, _] = volume.dims().map(|count| count as usize);
    let [block_cols, block_rows, block_slices] = dims.map(|count| count as usize);
    let block = block as usize;
    let mut maxima = vec![f32::NEG_INFINITY; block_cols * block_rows * block_slices];
    for (slice, slab) in volume.values().chunks(cols * rows).enumerate() {
        for (row, line) in slab.chunks(cols).enumerate() {
            let first = (slice / block * block_rows + row / block) * block_cols;
            for (col, value) in line.iter().enumerate() {
                if let Some(largest) = maxima.get_mut(first + col / block) {
                    *largest = largest.max(*value);
                }
            }
        }
    }
    (maxima, dims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::filled;

    #[test]
    fn a_halved_volume_holds_the_mean_of_each_block() {
        let half = halved(&filled([4, 4, 4], |col, _, _| f32::from(col)));
        assert_eq!(half.dims(), [2, 2, 2]);
        assert_eq!(half.values(), [0.5, 2.5, 0.5, 2.5, 0.5, 2.5, 0.5, 2.5]);
    }

    #[test]
    fn a_halved_volume_fills_the_same_place_in_the_patient() {
        let placed = Affine::from_columns(
            [0.5, 0.0, 0.0],
            [0.0, 0.4, 0.3],
            [0.0, -1.8, 2.4],
            [-90.0, -120.0, 35.0],
        );
        let volume = Volume::new([4, 6, 2], vec![0.0; 48], placed).expect("adds up");
        let half = halved(&volume);
        assert_eq!(half.dims(), [2, 3, 1]);
        for (now, before) in half.corners().into_iter().zip(volume.corners()) {
            let apart =
                (now.x - before.x).abs() + (now.y - before.y).abs() + (now.z - before.z).abs();
            assert!(apart < 1e-9, "{now:?} was {before:?}");
        }
        for (spacing, expected) in half.spacing_mm().into_iter().zip([1.0, 1.0, 6.0]) {
            assert!(
                (spacing - expected).abs() < 1e-9,
                "{spacing} for {expected}"
            );
        }
    }

    #[test]
    fn an_odd_side_rounds_up_and_its_last_block_is_the_mean_of_what_is_there() {
        let half = halved(&filled([5, 4, 4], |col, _, _| f32::from(col)));
        assert_eq!(half.dims(), [3, 2, 2]);
        assert_eq!(half.values().get(..3), Some([0.5, 2.5, 4.0].as_slice()));
    }

    #[test]
    fn a_single_voxel_halves_to_itself() {
        let half = halved(&filled([1, 1, 1], |_, _, _| 7.0));
        assert_eq!((half.dims(), half.values()), ([1, 1, 1], [7.0].as_slice()));
    }

    #[test]
    fn halving_stops_when_the_volume_fits_the_side_and_the_bytes() {
        assert_eq!(
            halvings_needed([512, 512, 300], 256, u64::MAX),
            1,
            "too long a side"
        );
        assert_eq!(
            halvings_needed([512, 512, 64], 2048, 16 << 20),
            1,
            "too many bytes"
        );
        assert_eq!(halvings_needed([64, 64, 64], 2048, u64::MAX), 0);
        assert_eq!(
            halvings_needed([64, 64, 64], 2048, 64 * 64 * 64 * 2),
            0,
            "exactly fits"
        );
        assert_eq!(halvings_needed([64, 64, 64], 2048, 64 * 64 * 64 * 2 - 1), 1);
        assert_eq!(
            halvings_needed([1000, 10, 10], 100, u64::MAX),
            4,
            "1000, 500, 250, 125, 63"
        );
    }

    #[test]
    fn halving_stops_at_a_single_voxel_and_never_overflows() {
        assert_eq!(halvings_needed([1, 1, 1], 0, 0), 0);
        assert_eq!(halvings_needed([8, 1, 2], 0, 0), 3);
        // Two bytes for each of 2^96 voxels is more than can be counted; 2^60 voxels is not.
        assert_eq!(halvings_needed([u32::MAX; 3], u32::MAX, u64::MAX), 12);
    }

    #[test]
    fn each_block_holds_its_largest_value() {
        let spot = |col, row, slice| {
            if (col, row, slice) == (3, 3, 3) {
                9.0
            } else {
                0.0
            }
        };
        let (maxima, dims) = block_maxima(&filled([4, 4, 4], spot), 2);
        assert_eq!(dims, [2, 2, 2]);
        assert_eq!(maxima, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 9.0]);
    }

    #[test]
    fn blocks_follow_columns_then_rows_then_slices() {
        let volume = filled([4, 2, 2], |col, row, slice| {
            f32::from(col + 10 * row + 100 * slice)
        });
        let (maxima, dims) = block_maxima(&volume, 2);
        assert_eq!(dims, [2, 1, 1]);
        assert_eq!(maxima, [111.0, 113.0]);
        let (maxima, dims) = block_maxima(&volume, 1);
        assert_eq!((dims, maxima.as_slice()), ([4, 2, 2], volume.values()));
    }

    #[test]
    fn a_block_larger_than_the_volume_or_of_no_size_is_still_answered() {
        let volume = filled([3, 3, 3], |col, _, _| f32::from(col));
        assert_eq!(block_maxima(&volume, 16), (vec![2.0], [1, 1, 1]));
        assert_eq!(
            block_maxima(&volume, 0).1,
            [3, 3, 3],
            "taken as blocks of one voxel"
        );
    }

    #[test]
    fn values_that_are_not_numbers_are_left_out_of_a_block() {
        let values = vec![f32::NAN, 3.0, f32::NAN, f32::NAN];
        let volume = Volume::new([4, 1, 1], values, Affine::identity()).expect("adds up");
        assert_eq!(block_maxima(&volume, 2).0, [3.0, f32::NEG_INFINITY]);
    }
}
