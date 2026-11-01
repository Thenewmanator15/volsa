//! A volume cut along its slices into textures the device can hold: past the longest side
//! a 3D texture may have, or past what one texture may take, a long series goes into up to
//! four, in full detail. Neighbouring textures share one slice, so a value read between two
//! slices always lies within one texture; each but the last is a whole number of 8-slice
//! blocks long, so no block of largest values lies in two.

use super::CardRoom;

/// The most textures one volume is held in.
pub(crate) const MOST_SLABS: usize = 4;
/// The side of a block of largest values, in voxels: a slab but the last is a whole number
/// of them long.
const BLOCK: u32 = 8;

/// How a volume is cut into textures along its slices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlabPlan {
    /// Slices from the first of one texture to the first of the next.
    pub(crate) step: u32,
    /// Slices in each texture, the shared one included.
    pub(crate) depths: Vec<u32>,
}

impl SlabPlan {
    /// One texture holding all `slices`.
    pub(crate) fn whole(slices: u32) -> Self {
        Self {
            step: slices,
            depths: vec![slices],
        }
    }

    /// How a volume of `dims` is held on a device with `room`: in one texture when it fits,
    /// otherwise in slabs; `None` when it needs more than [`MOST_SLABS`], its slices are
    /// larger than a side the device allows, or it takes more than the device has room for.
    pub(crate) fn for_volume(dims: [u32; 3], room: &CardRoom) -> Option<Self> {
        let [cols, rows, slices] = dims;
        if cols == 0 || rows == 0 || slices == 0 {
            return None;
        }
        if cols > room.longest_side || rows > room.longest_side {
            return None;
        }
        let slice_bytes = u64::from(cols) * u64::from(rows) * 2;
        let total = slice_bytes * u64::from(slices);
        if room.most_bytes.is_some_and(|most| total > most) {
            return None;
        }
        let in_one = u64::from(room.longest_side).min(room.texture_bytes / slice_bytes);
        if u64::from(slices) <= in_one {
            return Some(Self::whole(slices));
        }
        let step = u32::try_from(in_one.saturating_sub(1) / u64::from(BLOCK) * u64::from(BLOCK))
            .ok()
            .filter(|step| *step > 0)?;
        let count = (slices - 1).div_ceil(step);
        if count as usize > MOST_SLABS {
            return None;
        }
        let depths = (0..count)
            .map(|slab| {
                if slab + 1 < count {
                    step + 1
                } else {
                    slices - slab * step
                }
            })
            .collect();
        Some(Self { step, depths })
    }

    /// How many textures.
    pub(crate) fn count(&self) -> usize {
        self.depths.len()
    }

    /// The slices texture `slab` holds, counted in the whole volume.
    pub(crate) fn slices(&self, slab: usize) -> std::ops::Range<u32> {
        let first = u32::try_from(slab).map_or(u32::MAX, |slab| slab.saturating_mul(self.step));
        let depth = self.depths.get(slab).copied().unwrap_or(0);
        first..first.saturating_add(depth)
    }

    /// What the shaders are told of the plan: the volume's slices, the step, the number of
    /// textures, and each texture's depth (1 for one not used).
    pub(crate) fn uniforms(&self) -> ([f32; 4], [f32; 4]) {
        let slices = self.slices(self.count().saturating_sub(1)).end;
        #[expect(
            clippy::cast_precision_loss,
            reason = "counts of slices and textures are far below 2^24"
        )]
        let float = |count: u32| count as f32;
        let mut depths = [1.0; 4];
        for (to, depth) in depths.iter_mut().zip(&self.depths) {
            *to = float(*depth);
        }
        let count = u32::try_from(self.count()).unwrap_or(1);
        ([float(slices), float(self.step), float(count), 0.0], depths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upload::MAX_TEXTURE_BYTES;

    fn room(longest_side: u32) -> CardRoom {
        CardRoom {
            longest_side,
            ..CardRoom::default()
        }
    }

    #[test]
    fn a_series_that_fits_is_one_slab() {
        let plan = SlabPlan::for_volume([512, 512, 300], &room(2048)).expect("fits");
        assert_eq!(plan, SlabPlan::whole(300));
        assert_eq!(
            plan.uniforms(),
            ([300.0, 300.0, 1.0, 0.0], [300.0, 1.0, 1.0, 1.0])
        );
    }

    #[test]
    fn the_study_ct_is_two_slabs_sharing_a_slice() {
        let plan = SlabPlan::for_volume([512, 512, 2000], &room(2048)).expect("fits");
        assert_eq!((plan.step, plan.depths.clone()), (1016, vec![1017, 984]));
        assert_eq!((plan.slices(0), plan.slices(1)), (0..1017, 1016..2000));
        assert!(plan.step.is_multiple_of(BLOCK));
    }

    #[test]
    fn a_long_thin_series_needs_four_and_five_would_be_too_many() {
        let thin = room(64);
        let four = SlabPlan::for_volume([32, 32, 225], &thin).expect("fits in four");
        assert_eq!((four.step, four.count()), (56, 4));
        assert_eq!(four.slices(3), 168..225);
        assert!(SlabPlan::for_volume([32, 32, 226], &thin).is_none(), "five");
    }

    #[test]
    fn series_at_the_edges_of_a_slab_are_cut_where_they_should_be() {
        let thin = room(17);
        assert_eq!(
            SlabPlan::for_volume([4, 4, 2], &thin),
            Some(SlabPlan::whole(2))
        );
        assert_eq!(
            SlabPlan::for_volume([4, 4, 17], &thin),
            Some(SlabPlan::whole(17))
        );
        let one_more = SlabPlan::for_volume([4, 4, 18], &thin).expect("two");
        assert_eq!((one_more.step, one_more.depths.clone()), (16, vec![17, 2]));
        let fill = SlabPlan::for_volume([4, 4, 33], &thin).expect("two");
        assert_eq!(
            fill.depths,
            [17, 17],
            "the last slab ends on the last slice"
        );
    }

    #[test]
    fn a_slice_wider_than_the_device_allows_or_too_little_room_has_no_plan() {
        assert!(SlabPlan::for_volume([4096, 512, 10], &room(2048)).is_none());
        let small = CardRoom {
            most_bytes: Some(1 << 20),
            ..room(2048)
        };
        assert!(SlabPlan::for_volume([512, 512, 10], &small).is_none());
        let bytes = CardRoom {
            texture_bytes: MAX_TEXTURE_BYTES,
            ..room(16384)
        };
        let ct = SlabPlan::for_volume([512, 512, 2000], &bytes).expect("fits");
        assert_eq!(
            ct.count(),
            2,
            "512 MiB a texture, even where the side allows more"
        );
    }
}
