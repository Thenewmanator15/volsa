//! A label set: a label volume with its numbers turned into named parts, each with its
//! volume. This is what the rest of Voxa shows.

use std::collections::HashMap;

use voxa_core::{Affine, Patient, Voxel};

use crate::nifti::LabelGrid;
use crate::table::{Side, Table, Trust, Verdict, colour_for};

/// The group that parts with no name in the table are listed under.
pub const NOT_NAMED: &str = "Not named in the table";
/// The most parts a set may have: part numbers are held in 16 bits, and 0 is nothing.
const MAX_PARTS: usize = 65_535;

/// One named structure on one side, as it is in a label set.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// What it is called.
    pub name: String,
    /// The group it is listed under; empty when the table has none.
    pub group: String,
    /// Which side of the patient it is on.
    pub side: Side,
    /// The colour it is drawn in: red, green, blue.
    pub colour: [u8; 3],
    /// How far its name can be trusted.
    pub trust: Trust,
    /// How many voxels of the volume it takes up.
    pub voxels: u64,
    /// Its volume in millilitres: its voxels times the size of one.
    pub ml: f64,
}

/// A label volume as named parts. `numbers` holds, for every voxel in the order stored, 0
/// for nothing or 1 more than the part's place in `parts`.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelSet {
    /// Columns, rows and slices.
    pub dims: [u32; 3],
    /// The part at each voxel, columns fastest.
    pub numbers: Vec<u16>,
    /// Where each voxel lies in the patient.
    pub placement: Affine<Voxel, Patient>,
    /// The parts, those the table names first, in its order.
    pub parts: Vec<Part>,
    /// What the labels are of, as their table says; empty when there is none.
    pub title: String,
    /// The labelling model's own check, when the table carries one.
    pub verdict: Option<Verdict>,
}

/// Why a label volume cannot be made into a set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SetError {
    /// It holds more different numbers than Voxa lists.
    #[error("These labels have more different parts than Voxa can list.")]
    TooManyParts,
}

/// Make a label volume into named parts. A name of the table whose numbers are not in the
/// volume makes no part; a number in the volume the table does not name makes a part called
/// after the number, in the group [`NOT_NAMED`], so that nothing in the file is left out
/// unseen. A number two names claim belongs to the first. With no table every part is named
/// after its number.
///
/// # Errors
///
/// [`SetError::TooManyParts`] when the volume holds more than 65 535 different numbers.
pub fn label_set(grid: &LabelGrid, table: Option<&Table>) -> Result<LabelSet, SetError> {
    let counted = counted(&grid.ids)?;
    let voxel_ml = voxel_volume(&grid.placement) / 1000.0;
    let (parts, number_of) = parts_of(&counted, table, voxel_ml);
    let number = |id: &u32| number_of.get(id).copied().unwrap_or(0);
    Ok(LabelSet {
        dims: grid.dims,
        numbers: grid.ids.iter().map(number).collect(),
        placement: grid.placement,
        parts,
        title: table.map_or_else(String::new, |table| table.title.clone()),
        verdict: table.and_then(|table| table.verdict.clone()),
    })
}

/// How many voxels hold each number other than 0.
fn counted(ids: &[u32]) -> Result<HashMap<u32, u64>, SetError> {
    let mut counted: HashMap<u32, u64> = HashMap::new();
    // Labels come in long runs of one number: a run is counted, then noted once.
    let mut run: Option<(u32, u64)> = None;
    for id in ids {
        match &mut run {
            Some((of, count)) if of == id => *count += 1,
            _ => {
                if let Some((of, count)) = run.replace((*id, 1)) {
                    *counted.entry(of).or_default() += count;
                }
                if counted.len() > MAX_PARTS + 1 {
                    return Err(SetError::TooManyParts);
                }
            }
        }
    }
    if let Some((of, count)) = run {
        *counted.entry(of).or_default() += count;
    }
    counted.remove(&0);
    if counted.len() > MAX_PARTS {
        return Err(SetError::TooManyParts);
    }
    Ok(counted)
}

/// The parts of a volume whose numbers, with how many voxels each has, are `counted`; and
/// which part each number belongs to, counting parts from 1.
fn parts_of(
    counted: &HashMap<u32, u64>,
    table: Option<&Table>,
    voxel_ml: f64,
) -> (Vec<Part>, HashMap<u32, u16>) {
    let mut parts: Vec<Part> = Vec::new();
    let mut number_of: HashMap<u32, u16> = HashMap::new();
    let next = |parts: &Vec<Part>| u16::try_from(parts.len() + 1).unwrap_or(u16::MAX);
    let entries = table.map_or(&[][..], |table| table.entries.as_slice());
    for entry in entries {
        let mut voxels = 0;
        for id in &entry.ids {
            let unclaimed = counted.get(id).filter(|_| !number_of.contains_key(id));
            if let Some(count) = unclaimed {
                voxels += count;
                number_of.insert(*id, next(&parts));
            }
        }
        if voxels > 0 {
            parts.push(Part {
                name: entry.name.clone(),
                group: entry.group.clone(),
                side: entry.side,
                colour: entry.colour,
                trust: entry.trust.clone(),
                voxels,
                ml: voxels as f64 * voxel_ml,
            });
        }
    }
    let unclaimed = counted.iter().filter(|(id, _)| !number_of.contains_key(id));
    let mut unnamed: Vec<(u32, u64)> = unclaimed.map(|(id, count)| (*id, *count)).collect();
    unnamed.sort_unstable();
    for (id, voxels) in unnamed {
        number_of.insert(id, next(&parts));
        parts.push(Part {
            name: format!("Label {id}"),
            group: NOT_NAMED.to_owned(),
            side: Side::Midline,
            colour: colour_for(id),
            trust: Trust::NotNamed,
            voxels,
            ml: voxels as f64 * voxel_ml,
        });
    }
    (parts, number_of)
}

/// The volume of one voxel, in cubic millimetres: the size of the box its three steps span.
fn voxel_volume(placement: &Affine<Voxel, Patient>) -> f64 {
    let [[ax, ay, az], [bx, by, bz], [cx, cy, cz]] = placement.columns();
    let spanned = ax * (by * cz - bz * cy) - ay * (bx * cz - bz * cx) + az * (bx * cy - by * cx);
    spanned.abs()
}
