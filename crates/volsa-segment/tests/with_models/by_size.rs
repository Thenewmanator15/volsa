//! The bound on what a whole run asks the system for, held where the size of the scan decides
//! it: the made-up head of the check folder in a grid of air as large as a whole-head scan,
//! run on the processor with the robust model and the areas of the cortex. There, what the
//! labelling networks ask for grows with the scan and is more than the scores network's
//! figure, which is the same for any scan; on the check head alone it is about as much.

use voxa_core::{Affine, Volume};
use voxa_models::{Cut, Device};
use voxa_segment::label;
use voxa_segment::testing::Bound;

use super::{CHECK, REAL, ROBUST, on, turn};

/// The grid of air the head is put in the middle of, in voxels: given to the networks, it is
/// padded to 192 a side.
const AIR: [usize; 3] = [176, 176, 176];
/// What the cortex network asks of the processor for each voxel, and what the scores network
/// asks of it whatever the scan, in bytes: `voxa-models`' own figures, written out here so
/// that the test says which is the larger.
const CORTEX_A_VOXEL: u64 = 900;
const SCORES: u64 = 31 * (1 << 30) / 10;

/// `head` in the middle of a grid of air of `sides`, placed so that each of its voxels lies
/// in the patient where it lay before.
fn in_air(head: &Volume, sides: [usize; 3]) -> Volume {
    let [cols, rows, slices] = head.dims().map(|side| side as usize);
    let from = [0, 1, 2].map(|axis| (sides[axis] - [cols, rows, slices][axis]) / 2);
    let mut values = vec![0.0; sides.iter().product()];
    for slice in 0..slices {
        for row in 0..rows {
            let at = ((slice + from[2]) * sides[1] + row + from[1]) * sides[0] + from[0];
            let first = (slice * rows + row) * cols;
            values[at..at + cols].copy_from_slice(&head.values()[first..first + cols]);
        }
    }
    let placed = head.voxel_to_patient();
    let [x, y, z] = placed.columns();
    let origin = placed.origin();
    let moved = [0, 1, 2].map(|row| {
        let step = |column: [f64; 3], by: usize| column[row] * by as f64;
        origin[row] - step(x, from[0]) - step(y, from[1]) - step(z, from[2])
    });
    let placed = Affine::from_columns(x, y, z, moved);
    let sides = sides.map(|side| u32::try_from(side).expect("a side"));
    Volume::new(sides, values, placed).expect("a volume")
}

#[test]
#[ignore = "needs VOXA_RUNTIME, VOXA_MODELS and 10 GiB of memory"]
fn a_whole_run_on_a_scan_the_size_of_a_whole_head_asks_the_processor_for_no_more_than_its_bound() {
    let _turn = turn();
    let lists = &REAL.1.lists;
    let head = CHECK.volume("volume", "volume.f32");
    let volume = in_air(&head, AIR);
    let sizes = voxa_segment::sizes(&volume).expect("the sizes of the scan");
    let voxels = usize::try_from(sizes.whole).expect("a count");
    assert_eq!(voxels, 192 * 192 * 192, "the image the networks are given");
    assert!(
        sizes.whole * CORTEX_A_VOXEL > SCORES,
        "the labelling networks' figure is the larger, so the scan's size decides the bound"
    );
    // Beside the run the test holds the volume and the head it was made of.
    let arrays = [
        ("the volume", volume.values().len()),
        ("the head", head.values().len()),
    ];
    let bound = Bound::new(voxels, ROBUST, &arrays);
    let processor = (Device::Processor, "the processor");
    let what = "a run on a scan the size of a whole head";
    let labelled = on(processor, what, &bound, |networks, said| {
        let labelled = label(&volume, Cut::Whole, lists, ROBUST, networks, said);
        labelled.expect("the scan is labelled")
    });
    let verdict = labelled.table.verdict.as_ref();
    println!("  the model's own check: {verdict:?}");
}
