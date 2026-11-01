// Ported from SynthSeg (https://github.com/BBillot/SynthSeg), Copyright 2020 Benjamin Billot,
// Apache License 2.0: `predict` in SynthSeg/predict_synthseg.py, translated to Rust and changed for
// Volsa.
//! From a volume all the way to a label set: the volume is made ready, the networks are run,
//! what they found is tidied, and the table that names it is made. Each step is the one
//! `SynthSeg`'s own program takes for the networks that were asked for: the standard network's
//! answer is tidied its standard way, and the robust networks' the way it calls fast.
//!
//! Which model runs and whether the scan is cut to the head are for the caller to settle.
//! [`choose`] holds the one rule there is for the model. What a reader of the result should
//! know of the run is said in notes: that slices far apart make a rough map, and that the
//! scan was cut to the head.

use volsa_core::Volume;
use volsa_labels::{LabelGrid, Table};
use volsa_models::{Cut, Lists, RunError, Wanted};

use crate::find::{Networks, Stage, find};
use crate::prepare::{Back, PrepareError, Prepared, prepare};
use crate::table::table;
use crate::tidy::tidy;

/// The robust model is picked for a scan whose voxels are further apart than this along any
/// of its axes, in millimetres; and labels made from such a scan are a rough map.
pub const THICK_MM: f64 = 3.0;
/// The note on a scan that was cut to the head. It says what was measured. A cut lays the
/// head elsewhere in the box the networks are given, and that alone moves their answer: on a
/// made-up head about one labelled voxel in a hundred came out otherwise, each of them where
/// two parts meet or next to it, all through the head, and `SynthSeg`'s own program did the
/// same when given the head laid otherwise. Neither answer is the truer. The figures are
/// kept with the project's findings on labelling, and go from there into its decision record.
const CUT_NOTE: &str = "The scan was cut to the head to fit. Where two parts meet, some voxels \
                        may be labelled differently than without the cut.";

/// Which model a person has asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelChoice {
    /// The robust model for a scan with slices far apart, the standard one otherwise.
    Automatic,
    /// The standard model, whatever the scan.
    Standard,
    /// The robust model, whatever the scan.
    Robust,
}

/// Whether the robust model is the one to run: true for robust. `spacing_mm` is the distance
/// between neighbouring voxels of the scan as it was given, along its columns, rows and
/// slices. [`ModelChoice::Automatic`] takes the robust model when any of the three is over
/// [`THICK_MM`].
pub fn choose(spacing_mm: [f64; 3], choice: ModelChoice) -> bool {
    match choice {
        ModelChoice::Automatic => spacing_mm.iter().any(|&mm| mm > THICK_MM),
        ModelChoice::Standard => false,
        ModelChoice::Robust => true,
    }
}

/// A volume's labels, with what names them and what a reader should know of them.
#[derive(Debug, Clone, PartialEq)]
pub struct Labelled {
    /// The label of each voxel of the volume's 1 mm grid, with the grid's place in the
    /// patient: at least one voxel has a label.
    pub grid: LabelGrid,
    /// The name of every label there can be, and the model's verdict on its own work.
    pub table: Table,
    /// What a reader should know of how the labels were made, each a sentence: first that the
    /// map is rough, then that the scan was cut. Empty when there is nothing to say.
    pub notes: Vec<String>,
}

/// Why a volume was not labelled. Each says so in words a person can be shown.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
    /// The volume could not be made ready for the networks.
    #[error(transparent)]
    Prepare(#[from] PrepareError),
    /// A network gave no answer, or one that could not be used, or there was not the memory
    /// for what is made of the answers.
    #[error(transparent)]
    Run(#[from] RunError),
    /// The networks ran, and no voxel of the scan was given a label.
    #[error("The model found no brain in this scan.")]
    NothingFound,
}

/// Labels a volume: makes it ready, runs the networks on it, tidies what they found and
/// names it.
///
/// The three robust networks stand in for the standard one when `wanted.robust` is set, and
/// the cortex is divided into its areas when `wanted.cortex` is. What the robust networks
/// find is tidied the way `SynthSeg`'s own program tidies it, which that program calls fast;
/// what the standard network finds, the standard way. [`tidy`] says how the two differ.
/// `cut` says whether the networks are given the whole scan or only the head. `said` is
/// told each stage before it begins: [`Stage::Preparing`], the stages of [`find`], then
/// [`Stage::Tidying`].
///
/// The notes say that the map is rough when the scan's voxels are over [`THICK_MM`] apart
/// along any axis, naming the longest of the three distances; and that the scan was cut to
/// the head, when the cut took something away.
///
/// # Errors
///
/// [`LabelError::Prepare`] when the volume cannot be made ready, [`LabelError::Run`] when a
/// network fails or memory runs out, each with the reason. [`LabelError::NothingFound`] when
/// all went well and no voxel of the scan has a label.
pub fn label(
    volume: &Volume,
    cut: Cut,
    lists: &Lists,
    wanted: Wanted,
    networks: &mut dyn Networks,
    said: &mut dyn FnMut(Stage),
) -> Result<Labelled, LabelError> {
    said(Stage::Preparing);
    let Prepared {
        image,
        back,
        spacing_mm,
    } = prepare(volume, cut)?;
    let found = find(&image, lists, wanted, networks, said)?;
    // The image has done its work, and the tidying needs the room.
    drop(image);
    said(Stage::Tidying);
    let grid = tidy(&found, lists, wanted, &back)?;
    if grid.ids.iter().all(|&id| id == 0) {
        return Err(LabelError::NothingFound);
    }
    Ok(Labelled {
        grid,
        table: table(lists, wanted, &found.scores),
        notes: notes(spacing_mm, cut_away(&back)),
    })
}

/// Whether a cut took anything away: the image is shorter than the grid along some axis.
/// A cut to a head that fills the grid takes nothing, and neither does no cut at all.
fn cut_away(back: &Back) -> bool {
    let shorter = |(&size, &from): (&usize, &usize)| {
        let side = back.grid_dims.get(from).copied();
        side.is_some_and(|side| usize::try_from(side).is_ok_and(|side| size < side))
    };
    back.size.iter().zip(&back.order).any(shorter)
}

/// What a reader should know of labels made from a scan with voxels this far apart, of which
/// a cut took something away or did not.
pub(crate) fn notes(spacing_mm: [f64; 3], cut_away: bool) -> Vec<String> {
    let mut notes = Vec::new();
    let longest = spacing_mm.iter().copied().fold(0.0, f64::max);
    if longest > THICK_MM {
        let apart = in_mm(longest);
        notes.push(format!(
            "The slices of this scan are {apart} mm apart, so this is a rough map."
        ));
    }
    if cut_away {
        notes.push(CUT_NOTE.to_owned());
    }
    notes
}

/// A distance in millimetres as it is written: to one decimal place, and with none when
/// that place is nought.
fn in_mm(mm: f64) -> String {
    let written = format!("{mm:.1}");
    match written.strip_suffix(".0") {
        Some(whole) => whole.to_owned(),
        None => written,
    }
}
