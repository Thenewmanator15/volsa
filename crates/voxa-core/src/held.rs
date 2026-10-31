//! A series held as its values are stored: 16-bit whole numbers with one rescale, or 32-bit
//! numbers where whole numbers cannot give the values exactly, in slabs of whole slices.
//! Half the memory of 32-bit values for a scan stored in 16 bits, and the same values to the
//! bit. Slices are put in from many threads at once, each straight into its place.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, PoisonError, RwLock};

use crate::grid::Grid;
use crate::rescale::Rescale;
use crate::space::{Affine, Patient, Point, Voxel};
use crate::volume::Volume;

mod away;
mod values;

use away::Slab;
pub use away::SlabStore;
use values::Values;

/// How a held volume keeps its values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    /// Unsigned 16-bit whole numbers, with the volume's rescale.
    Unsigned16,
    /// Signed 16-bit whole numbers, with the volume's rescale.
    Signed16,
    /// The values themselves, as 32-bit numbers.
    Float32,
}

impl Stored {
    /// The bytes one value takes.
    pub fn bytes(self) -> usize {
        match self {
            Self::Unsigned16 | Self::Signed16 => 2,
            Self::Float32 => 4,
        }
    }
}

/// The numbers of one slice to put in a held volume, of the kind it holds.
#[derive(Debug, Clone, Copy)]
pub enum SliceIn<'a> {
    /// Unsigned whole numbers.
    Unsigned(&'a [u16]),
    /// Signed whole numbers.
    Signed(&'a [i16]),
    /// Values.
    Float(&'a [f32]),
}

/// The numbers of one slab of a held volume, as held: its slices one after another.
#[derive(Debug, Clone, Copy)]
pub enum SlabRaw<'a> {
    /// Unsigned whole numbers.
    Unsigned(&'a [u16]),
    /// Signed whole numbers.
    Signed(&'a [i16]),
    /// Values.
    Float(&'a [f32]),
}

/// Why a slice could not be put in a held volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HeldError {
    /// The slice's numbers are not of the kind the volume holds.
    #[error("the slice is not of the kind the volume holds")]
    WrongKind,
    /// The slice does not hold a slice's worth of numbers.
    #[error("the slice is not the size of the volume's slices")]
    WrongSize,
    /// The volume has no slice of that number.
    #[error("the volume has no such slice")]
    NoSuchSlice,
}

/// A series' values as stored, in slabs of whole slices, with the transform that places each
/// voxel in the patient. Slices are put in through a shared reference, so that many threads
/// can put theirs at once; a slab is made when its first slice is put.
pub struct HeldVolume {
    dims: [u32; 3],
    voxel_to_patient: Affine<Voxel, Patient>,
    patient_to_voxel: Affine<Patient, Voxel>,
    stored: Stored,
    rescale: Rescale,
    /// Slices in a slab; the last slab may have fewer.
    per_slab: u32,
    slabs: Vec<RwLock<Slab>>,
    /// Which slices have been put.
    put: Vec<AtomicBool>,
    count: AtomicU32,
    /// When each slab was last read, by `clock`, so that the least used is put away first.
    read_at: Vec<AtomicU64>,
    clock: AtomicU64,
    /// Where slabs are put away past the budget, once there is somewhere.
    away: OnceLock<Arc<dyn SlabStore>>,
}

impl HeldVolume {
    /// An empty volume of `dims` (columns, rows, slices) placed by `voxel_to_patient`, holding
    /// its values as `stored` with `rescale`, in slabs of as many whole slices as fit in
    /// `slab_bytes`, one at least. `None` when a dimension is 0, the volume has more values
    /// than can be counted, or the transform cannot be undone.
    pub fn new(
        dims: [u32; 3],
        voxel_to_patient: Affine<Voxel, Patient>,
        stored: Stored,
        rescale: Rescale,
        slab_bytes: usize,
    ) -> Option<Self> {
        let [cols, rows, slices] = dims.map(|count| count as usize);
        let in_slice = cols.checked_mul(rows)?;
        if in_slice.checked_mul(slices)? == 0 {
            return None;
        }
        let patient_to_voxel = voxel_to_patient.inverse()?;
        let fit = slab_bytes / in_slice.checked_mul(stored.bytes())?;
        let per_slab = u32::try_from(fit.clamp(1, slices)).ok()?;
        let slab_count = dims[2].div_ceil(per_slab) as usize;
        Some(Self {
            dims,
            voxel_to_patient,
            patient_to_voxel,
            stored,
            rescale,
            per_slab,
            slabs: (0..slab_count).map(|_| RwLock::new(Slab::Empty)).collect(),
            put: (0..slices).map(|_| AtomicBool::new(false)).collect(),
            count: AtomicU32::new(0),
            read_at: (0..slab_count).map(|_| AtomicU64::new(0)).collect(),
            clock: AtomicU64::new(0),
            away: OnceLock::new(),
        })
    }

    /// How the values are held.
    pub fn stored(&self) -> Stored {
        self.stored
    }

    /// The rescale that turns a stored number into its value.
    pub fn rescale(&self) -> Rescale {
        self.rescale
    }

    /// How many slabs the volume is held in.
    pub fn slab_count(&self) -> usize {
        self.slabs.len()
    }

    /// The slices a slab holds.
    pub fn slab_slices(&self, slab: usize) -> Range<u32> {
        let first = u32::try_from(slab).map_or(u32::MAX, |slab| slab.saturating_mul(self.per_slab));
        first.min(self.dims[2])..first.saturating_add(self.per_slab).min(self.dims[2])
    }

    fn in_slice(&self) -> usize {
        self.dims[0] as usize * self.dims[1] as usize
    }

    /// Put a slice's numbers in their place.
    ///
    /// # Errors
    ///
    /// [`HeldError`] when there is no such slice, or the numbers are not a slice's worth of
    /// the kind the volume holds.
    pub fn put_slice(&self, slice: u32, numbers: SliceIn<'_>) -> Result<(), HeldError> {
        let len = match numbers {
            SliceIn::Unsigned(from) => from.len(),
            SliceIn::Signed(from) => from.len(),
            SliceIn::Float(from) => from.len(),
        };
        let flag = self.put.get(slice as usize).ok_or(HeldError::NoSuchSlice)?;
        if len != self.in_slice() {
            return Err(HeldError::WrongSize);
        }
        let slab = (slice / self.per_slab) as usize;
        let cell = self.slabs.get(slab).ok_or(HeldError::NoSuchSlice)?;
        let mut held = cell.write().unwrap_or_else(PoisonError::into_inner);
        if matches!(*held, Slab::Empty) {
            let slices = self.slab_slices(slab).len();
            *held = Slab::Here(Values::zeroed(self.stored, slices * self.in_slice()));
        }
        // Only a whole slab is put away, and nothing more is put in a whole one.
        let Slab::Here(made) = &mut *held else {
            return Err(HeldError::NoSuchSlice);
        };
        made.put((slice % self.per_slab) as usize * self.in_slice(), numbers)?;
        drop(held);
        if !flag.swap(true, Ordering::SeqCst) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    /// Whether a slice has been put.
    pub fn has_slice(&self, slice: u32) -> bool {
        self.put
            .get(slice as usize)
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
    }

    /// Whether every slice has been put.
    pub fn is_complete(&self) -> bool {
        self.count.load(Ordering::SeqCst) == self.dims[2]
    }

    /// The values of a slice that has been put, row by row.
    pub fn slice_values(&self, slice: u32) -> Option<Vec<f32>> {
        if !self.has_slice(slice) {
            return None;
        }
        let mut values = Vec::with_capacity(self.in_slice());
        self.extend_with_slice(slice, &mut values)?;
        Some(values)
    }

    /// Put the values of a slice that has been put on the end of `into`, a whole slice at a
    /// time. `None`, with nothing put, when it has not been put.
    fn extend_with_slice(&self, slice: u32, into: &mut Vec<f32>) -> Option<()> {
        if !self.has_slice(slice) {
            return None;
        }
        let first = (slice % self.per_slab) as usize * self.in_slice();
        let range = first..first + self.in_slice();
        let rescale = self.rescale;
        self.read_slab((slice / self.per_slab) as usize, |values| {
            match values {
                Values::Unsigned(numbers) => {
                    let numbers = numbers.get(range)?;
                    into.extend(numbers.iter().map(|n| rescale.value(i32::from(*n))));
                }
                Values::Signed(numbers) => {
                    let numbers = numbers.get(range)?;
                    into.extend(numbers.iter().map(|n| rescale.value(i32::from(*n))));
                }
                Values::Float(numbers) => into.extend_from_slice(numbers.get(range)?),
            }
            Some(())
        })?
    }

    /// Read a slab's numbers as held, if it has been made.
    pub fn with_slab<T>(&self, slab: usize, read: impl FnOnce(SlabRaw<'_>) -> T) -> Option<T> {
        self.read_slab(slab, |values| read(values.raw()))
    }

    /// Read one slice's numbers as held, if its slab has been made.
    pub fn with_slice<T>(&self, slice: u32, read: impl FnOnce(SlabRaw<'_>) -> T) -> Option<T> {
        if slice >= self.dims[2] {
            return None;
        }
        let first = (slice % self.per_slab) as usize * self.in_slice();
        let range = first..first + self.in_slice();
        self.read_slab((slice / self.per_slab) as usize, |values| {
            let raw = match values {
                Values::Unsigned(numbers) => SlabRaw::Unsigned(numbers.get(range)?),
                Values::Signed(numbers) => SlabRaw::Signed(numbers.get(range)?),
                Values::Float(numbers) => SlabRaw::Float(numbers.get(range)?),
            };
            Some(read(raw))
        })?
    }

    /// The values of a range of slices that have been put, as a volume of 32-bit numbers
    /// placed where they lie: its first slice is the range's first. `None` for an empty
    /// range, one past the last slice, or one holding a slice not put.
    pub fn to_volume(&self, slices: Range<u32>) -> Option<Volume> {
        if slices.is_empty() || slices.end > self.dims[2] {
            return None;
        }
        let mut values = Vec::with_capacity(slices.len() * self.in_slice());
        for slice in slices.clone() {
            self.extend_with_slice(slice, &mut values)?;
        }
        let [columns, rows, across] = self.voxel_to_patient.columns();
        let first = Point::new(0.0, 0.0, f64::from(slices.start));
        let at = self.voxel_to_patient.apply(first);
        let placed = Affine::from_columns(columns, rows, across, [at.x, at.y, at.z]);
        let dims = [
            self.dims[0],
            self.dims[1],
            u32::try_from(slices.len()).ok()?,
        ];
        Volume::new(dims, values, placed)
    }

    /// The bytes of the slabs made so far that are in memory, and not put away.
    pub fn bytes_held(&self) -> usize {
        let held = |cell: &RwLock<Slab>| match &*cell.read().unwrap_or_else(PoisonError::into_inner)
        {
            Slab::Here(values) => values.bytes(),
            Slab::Empty | Slab::Away => 0,
        };
        self.slabs.iter().map(held).sum()
    }
}

impl Grid for HeldVolume {
    fn dims(&self) -> [u32; 3] {
        self.dims
    }

    fn voxel_to_patient(&self) -> Affine<Voxel, Patient> {
        self.voxel_to_patient
    }

    fn patient_to_voxel(&self) -> Affine<Patient, Voxel> {
        self.patient_to_voxel
    }

    fn value(&self, col: u32, row: u32, slice: u32) -> Option<f32> {
        let [cols, rows, slices] = self.dims;
        if col >= cols || row >= rows || slice >= slices {
            return None;
        }
        let within = (slice % self.per_slab) as usize * self.in_slice();
        let at = within + row as usize * cols as usize + col as usize;
        let slab = (slice / self.per_slab) as usize;
        self.read_slab(slab, |values| values.value(at, self.rescale))?
    }
}

impl std::fmt::Debug for HeldVolume {
    /// Prints the size and how the values are held, not the values: there are millions.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeldVolume")
            .field("dims", &self.dims)
            .field("stored", &self.stored)
            .field("rescale", &self.rescale)
            .field("per_slab", &self.per_slab)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod away_tests;
#[cfg(test)]
mod tests;
