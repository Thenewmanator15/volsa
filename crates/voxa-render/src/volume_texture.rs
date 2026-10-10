//! A volume on the graphics device: its values as a 3D texture, reduced if it has to be, and
//! beside it the largest value in each block of voxels, for stepping over empty space.

use std::borrow::Cow;

use egui_wgpu::wgpu;
use voxa_core::{Affine, Patient, Volume, Voxel};
use voxa_volume::halved;

pub(crate) use crate::prepared::BLOCK;
use crate::prepared::Prepared;

/// The most bytes of slices handed to the device in one piece.
const SLAB_BYTES: usize = 16 << 20;
const BYTES_PER_VOXEL: u32 = 2;

/// A volume as the graphics device holds it, and what is needed to find a point of the
/// patient in it and to turn a stored value back into the scan's own.
pub(crate) struct Placed {
    /// The two textures themselves, kept so that their memory can be given back at once.
    values: wgpu::Texture,
    largest: wgpu::Texture,
    /// The values as the shader reads them; the texture lasts as long as this does.
    pub(crate) view: wgpu::TextureView,
    /// The largest value in each block of [`BLOCK`] voxels a side and in the blocks touching
    /// it, stored as the values are.
    pub(crate) maxima: wgpu::TextureView,
    /// Columns, rows and slices as stored, after any halving.
    pub(crate) dims: [u32; 3],
    pub(crate) patient_to_voxel: Affine<Patient, Voxel>,
    /// The smallest distance between neighbouring voxels as stored, in millimetres.
    pub(crate) finest_mm: f64,
    /// The scan value stored as 0.
    pub(crate) smallest: f32,
    /// The span of scan values from the one stored as 0 to the one stored as 1.
    pub(crate) span: f32,
}

impl Placed {
    /// Put a volume on the graphics device as it is. `None` when the device has no room.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue, volume: &Volume) -> Option<Self> {
        Self::from_prepared(device, queue, &Prepared::as_it_is(volume, 0))
    }

    /// As [`Placed::new`], with the largest scan value in each block, and the number of
    /// blocks each way, given and not worked out.
    #[cfg(test)]
    pub(crate) fn with_maxima(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        volume: &Volume,
        maxima: (&[f32], [u32; 3]),
    ) -> Option<Self> {
        Self::from_prepared(device, queue, &Prepared::with_maxima(volume, maxima, 0))
    }

    /// Put a volume that has been made ready on the graphics device: all that is left to do
    /// is hand over the bytes. `None` when the device has no room.
    pub(crate) fn from_prepared(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ready: &Prepared,
    ) -> Option<Self> {
        // Watched for running out of room from making the textures to the last byte handed
        // over: left unwatched, either would stop the program.
        let room = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let halves = wgpu::TextureFormat::R16Float;
        let values = texture(device, "voxa volume values", ready.dims, halves);
        let largest = texture(device, "voxa volume block maxima", ready.blocks, halves);
        write(device, queue, &values, ready.dims, &ready.halves);
        write(device, queue, &largest, ready.blocks, &ready.maxima);
        if pollster::block_on(room.pop()).is_some() {
            values.destroy();
            largest.destroy();
            return None;
        }
        Some(Self {
            view: values.create_view(&wgpu::TextureViewDescriptor::default()),
            maxima: largest.create_view(&wgpu::TextureViewDescriptor::default()),
            values,
            largest,
            dims: ready.dims,
            patient_to_voxel: ready.patient_to_voxel,
            finest_mm: ready.finest_mm,
            smallest: ready.smallest,
            span: ready.span,
        })
    }
}

impl Drop for Placed {
    /// Give the device its memory back now. Left to itself the device frees a texture some
    /// time later, and a second large volume put there in the meantime may not fit.
    fn drop(&mut self) {
        self.values.destroy();
        self.largest.destroy();
    }
}

/// A 3D texture of 16-bit numbers of one kind, `dims` texels each way.
pub(crate) fn texture(
    device: &wgpu::Device,
    label: &str,
    dims: [u32; 3],
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    let [width, height, depth_or_array_layers] = dims;
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

/// Hand the values to the device a slab of whole slices at a time, sending each on its way
/// before the next, so that the device never holds more than one slab waiting to be copied.
/// Values beyond the last whole slice, if there are any, are left out.
pub(crate) fn write(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    dims: [u32; 3],
    halves: &[u16],
) {
    let [cols, rows, slices] = dims;
    let in_a_slice = cols as usize * rows as usize;
    let slab_slices = (SLAB_BYTES / (in_a_slice * BYTES_PER_VOXEL as usize)).max(1);
    let starts = (0..slices).step_by(slab_slices);
    for (first, slab) in starts.zip(halves.chunks(in_a_slice * slab_slices)) {
        let whole_slices = slab.len() / in_a_slice;
        let (Some(slab), Ok(in_this_slab)) = (
            slab.get(..whole_slices * in_a_slice),
            u32::try_from(whole_slices),
        ) else {
            return;
        };
        if in_this_slab == 0 {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: first,
                },
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(slab),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(cols * BYTES_PER_VOXEL),
                rows_per_image: Some(rows),
            },
            wgpu::Extent3d {
                width: cols,
                height: rows,
                depth_or_array_layers: in_this_slab,
            },
        );
        // Send this slab on its way and let the device give back what carried the last.
        queue.submit(std::iter::empty());
        let _ = device.poll(wgpu::PollType::Poll);
    }
}

/// Halve `volume` `halvings` times and hand it to `place`. Each time `place` has no room for
/// it, halve it once more and try again, down to a single voxel. Gives what `place` made and
/// how many times the volume was halved; `None` when there was no room even for one voxel.
pub(crate) fn placed_by_halving<T>(
    volume: &Volume,
    halvings: u32,
    mut place: impl FnMut(&Volume) -> Option<T>,
) -> Option<(T, u32)> {
    let mut shown = Cow::Borrowed(volume);
    for _ in 0..halvings {
        shown = Cow::Owned(halved(&shown));
    }
    let mut halvings = halvings;
    loop {
        if let Some(placed) = place(&shown) {
            return Some((placed, halvings));
        }
        if shown.dims() == [1, 1, 1] {
            return None;
        }
        shown = Cow::Owned(halved(&shown));
        halvings += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepared::{as_halves, with_neighbours};

    /// 16-bit floating-point 0, a half and 1, as stored.
    const ZERO: u16 = 0x0000;
    const HALF: u16 = 0x3800;
    const ONE: u16 = 0x3C00;

    fn row_of(values: &[f32]) -> Volume {
        let count = u32::try_from(values.len()).expect("a few");
        Volume::new([count, 1, 1], values.to_vec(), Affine::identity()).expect("adds up")
    }

    fn cube(side: u32) -> Volume {
        let values = vec![0.0; (side * side * side) as usize];
        Volume::new([side; 3], values, Affine::identity()).expect("adds up")
    }

    #[test]
    fn values_are_stored_from_zero_at_the_smallest_to_one_at_the_largest() {
        let (stored, smallest, span) = as_halves(&row_of(&[10.0, 20.0, 30.0, -10.0]));
        assert_eq!(
            stored,
            [HALF, 0x3A00, ONE, ZERO],
            "a half, three quarters, one, nought"
        );
        assert_eq!((smallest, span), (-10.0, 40.0));
    }

    #[test]
    fn values_that_are_not_numbers_are_stored_at_an_end() {
        let odd = [0.0, 10.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
        let (stored, smallest, span) = as_halves(&row_of(&odd));
        assert_eq!(stored, [ZERO, ONE, ZERO, ONE, ZERO]);
        assert_eq!(
            (smallest, span),
            (0.0, 10.0),
            "the range is that of the numbers alone"
        );
    }

    #[test]
    fn a_volume_of_one_value_or_of_no_numbers_is_stored_as_zeros() {
        assert_eq!(
            as_halves(&row_of(&[5.0, 5.0])),
            (vec![ZERO, ZERO], 5.0, 1.0)
        );
        assert_eq!(
            as_halves(&row_of(&[f32::NAN, f32::NAN])),
            (vec![ZERO, ZERO], 0.0, 1.0)
        );
    }

    #[test]
    fn a_block_takes_in_the_largest_value_of_the_blocks_around_it() {
        assert_eq!(
            with_neighbours(&[0.0, 0.0, 5.0, 0.0, 0.0], [5, 1, 1]),
            [0.0, 5.0, 5.0, 5.0, 0.0]
        );
        // A spot in one corner of 3 x 3 x 2 blocks reaches the 2 x 2 x 2 blocks about it.
        let mut spot = vec![0.0; 18];
        spot[0] = 7.0;
        let spread = with_neighbours(&spot, [3, 3, 2]);
        let reached: Vec<usize> = (0..18).filter(|at| spread[*at] > 0.0).collect();
        assert_eq!(reached, [0, 1, 3, 4, 9, 10, 12, 13]);
    }

    #[test]
    fn spreading_does_not_run_on_from_the_end_of_one_row_into_the_next() {
        // 3 columns by 2 rows: the spot begins the second row, next in memory to the end of
        // the first.
        let spread = with_neighbours(&[0.0, 0.0, 0.0, 5.0, 0.0, 0.0], [3, 2, 1]);
        assert_eq!(spread, [5.0, 5.0, 0.0, 5.0, 5.0, 0.0]);
        let lone = with_neighbours(&[f32::NEG_INFINITY, 2.0], [2, 1, 1]);
        assert_eq!(
            lone,
            [2.0, 2.0],
            "a block of no numbers takes its neighbour's"
        );
    }

    #[test]
    fn a_volume_is_halved_as_often_as_asked_before_it_is_placed() {
        let whole = placed_by_halving(&cube(8), 0, |shown| Some(shown.dims()));
        assert_eq!(whole, Some(([8, 8, 8], 0)));
        let twice = placed_by_halving(&cube(8), 2, |shown| Some(shown.dims()));
        assert_eq!(twice, Some(([2, 2, 2], 2)));
    }

    #[test]
    fn a_volume_there_is_no_room_for_is_halved_until_there_is() {
        let room_for = |side: u32| {
            move |shown: &Volume| {
                shown
                    .dims()
                    .into_iter()
                    .all(|count| count <= side)
                    .then_some(())
            }
        };
        assert_eq!(placed_by_halving(&cube(8), 0, room_for(2)), Some(((), 2)));
        assert_eq!(placed_by_halving(&cube(8), 1, room_for(4)), Some(((), 1)));
        assert_eq!(
            placed_by_halving(&cube(8), 0, room_for(1)),
            Some(((), 3)),
            "one voxel"
        );
    }

    #[test]
    fn with_no_room_even_for_one_voxel_nothing_is_placed() {
        let mut tries = 0;
        let placed = placed_by_halving(&cube(8), 0, |_| {
            tries += 1;
            None::<()>
        });
        assert_eq!((placed, tries), (None, 4), "at 8, 4, 2 and 1 voxels a side");
    }
}
