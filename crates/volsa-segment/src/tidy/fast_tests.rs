//! Tests of tidying what the robust networks give, which `SynthSeg`'s own program tidies the
//! way it calls fast. The golden cases whose names begin `robust_` are what `SynthSeg`'s
//! tidying made of made-up chances when told so; the line of voxels is made up, and each
//! answer for it is the one `SynthSeg`'s tidying gives.

use super::tests::{ROBUST, STANDARD, count_of, lists, tidied_as_synthseg_tidies, tidied_line};

#[test]
fn a_structure_keeps_both_its_pieces_and_what_is_joined_through_the_padding_goes() {
    let case = tidied_as_synthseg_tidies("robust_pieces", ROBUST);
    // Read off SynthSeg's own labels. The left thalamus (10) has its 8 voxels and its 2. The
    // left ventral DC (28) is there, the right one (60) and the brain stem (16) are gone. And
    // of the right white matter (41) only the blob is left: 192 voxels less the thalamus's 10.
    // The two voxels joined to it through the padding went.
    let kept = [10, 28, 60, 16, 41].map(|label| count_of(&case, label));
    assert_eq!(kept, [10, 4, 0, 0, 182]);
}

#[test]
fn a_chance_other_than_the_first_must_be_over_a_fifth_to_stay() {
    let case = tidied_as_synthseg_tidies("robust_marks", ROBUST);
    // Left white matter (2) wins the voxel where it has 0.22 and the one where it has the next
    // number above 0.2, and no other. The right thalamus (49) stays and the right caudate (50)
    // goes; one voxel is right cortex (42).
    let kept = [2, 49, 50, 42].map(|label| count_of(&case, label));
    assert_eq!(kept, [2, 2, 0, 1]);
}

#[test]
fn the_cortex_of_a_robust_answer_takes_its_areas_and_a_cut_is_put_back() {
    let case = tidied_as_synthseg_tidies("robust_cortex", ROBUST);
    assert_eq!((count_of(&case, 3), count_of(&case, 42)), (0, 0));
    // The areas are labelled 1001 to 1035 on the left and 2001 to 2035 on the right.
    let labels = (1001..=1035).chain(2001..=2035);
    let areas: usize = labels.map(|label| count_of(&case, label)).sum();
    // Two voxels of left cortex lie apart from the rest of their class, in the right white
    // matter. The standard way they would go; this way they stay and take their areas.
    assert_eq!(
        areas, 50,
        "24 voxels of cortex on each side, and the 2 apart"
    );
    assert_ne!(case.back.cut_from, [0; 3], "the case has a cut");
}

/// Eight voxels of nothing, A and B. A, B and A again, joined; a voxel where A has 0.22 and
/// is the only chance over 0.2; one where A has exactly 0.2 and B a little less; B; nothing;
/// and B apart.
const EIGHT: [[f32; 3]; 8] = [
    [0.25, 0.75, 0.0],
    [0.25, 0.0, 0.75],
    [0.25, 0.75, 0.0],
    [0.21, 0.22, 0.19],
    [0.15, 0.2, 0.19],
    [0.25, 0.0, 0.75],
    [1.0, 0.0, 0.0],
    [0.25, 0.0, 0.75],
];

#[test]
fn the_same_chances_are_tidied_one_way_for_the_robust_networks_and_another_for_the_standard() {
    // A, labelled 7, has a class of its own; B, labelled 9, is of the lowest, with nothing.
    let (a, b) = (7, 9);
    let lists = lists(&[(0, 0), (a, 1), (b, 0)], &[], [3, 42]);
    // The fast way no class loses a piece, so A's second voxel stays. A's 0.22 is over 0.2
    // and stays, alone, so it wins; its 0.2 is not, and B's 0.19 neither, so that voxel is
    // nothing. The last voxel is not joined to the rest.
    let robust = tidied_line(&lists, ROBUST, &EIGHT, None);
    assert_eq!(robust, [a, b, a, a, 0, b, 0, 0]);
    // The standard way A keeps the first met of its two pieces and loses the other, with
    // every chance of 0.25 or less that it has elsewhere: where it had 0.22 nothing is the
    // largest left, and where it had 0.2, B.
    let standard = tidied_line(&lists, STANDARD, &EIGHT, None);
    assert_eq!(standard, [a, b, 0, 0, b, b, 0, 0]);
}

#[test]
fn the_classes_of_the_lists_make_no_difference_the_fast_way() {
    let (a, b) = (7, 9);
    for classes in [[1, 1, 1], [4, u32::MAX, 4], [9, 2, 9], [0, 1, 2]] {
        let [nothing, of_a, of_b] = classes;
        let lists = lists(&[(0, nothing), (a, of_a), (b, of_b)], &[], [3, 42]);
        let robust = tidied_line(&lists, ROBUST, &EIGHT, None);
        assert_eq!(robust, [a, b, a, a, 0, b, 0, 0], "{classes:?}");
    }
}
