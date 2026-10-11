//! Tests of the networks that find the structures: the standard one, run on the image and
//! on its mirror, and the three robust ones, run one after another.

use voxa_models::{Role, Tensor};

use super::Stage::{Checking, Finding};
use super::tests::{
    Answer, BOX, GRID, Shelf, floats, halves, image, lists, map, no_scores, plain_image, run,
    three, wanted,
};

/// A stand-in for the standard network that copies the image it is given into channel 1 of
/// three, and leaves the other two at nought.
#[expect(clippy::unnecessary_wraps, reason = "it answers as a network does")]
fn image_into_channel_one(inputs: &[Tensor]) -> Answer {
    let seen = floats(&inputs[0]).iter();
    let chances: Vec<f32> = seen.flat_map(|&value| [0.0, value, 0.0]).collect();
    Ok(halves(GRID, 3, 2, &chances))
}

#[test]
fn the_standard_answer_is_the_mean_of_the_answers_on_the_image_and_on_its_mirror() {
    // The image changes along x only: the four voxels at each x hold the same.
    let across = |values: [f32; 4]| image(GRID, values.iter().flat_map(|&v| [v; 4]).collect());
    let seen = across([0.25, 0.5, 0.75, 1.0]);
    let mut shelf = Shelf::default();
    let standard = shelf.put(Role::Standard, image_into_channel_one);
    shelf.put(Role::Scores, no_scores);
    let (found, stages) = run(&seen, &three(), wanted(false, false), &mut shelf);
    let found = found.expect("the image is labelled");

    // The second run is on the mirror: the image turned round along x.
    let mirror = across([1.0, 0.75, 0.5, 0.25]);
    assert_eq!(standard.runs(), vec![vec![seen], vec![mirror]]);
    // On the image the stand-in puts the image in channel 1. On the mirror it puts the
    // mirror there: turned back along x that is the image again, and the swap of the last
    // two channels moves it to channel 2. The mean leaves half the image in each of the
    // two. At each x, the three channels:
    let at_each_x: [[f32; 3]; 4] = [
        [0.0, 0.125, 0.125],
        [0.0, 0.25, 0.25],
        [0.0, 0.375, 0.375],
        [0.0, 0.5, 0.5],
    ];
    let expected: Vec<f32> = at_each_x
        .iter()
        .flat_map(|at_x| [*at_x; 4])
        .flatten()
        .collect();
    let expected = Tensor::f32(vec![1, 4, 2, 2, 3], expected).expect("whole");
    assert_eq!(found.probabilities, expected);
    assert_eq!(found.areas, None);
    let (first, second) = (Finding { step: 1, of: 2 }, Finding { step: 2, of: 2 });
    assert_eq!(stages, [first, second, Checking]);
    assert_eq!(
        shelf.opened,
        [(Role::Standard, GRID), (Role::Scores, [BOX; 3])]
    );
}

#[test]
fn the_image_and_the_answer_on_it_are_mirrored_along_x_only_and_the_channels_change_places() {
    // Two answers that differ at every voxel and in every channel. Each number is so many
    // 128ths, so every sum and every half is exact.
    let spread =
        |step: u16, more: u16| move |place: u16| f32::from(place * step % 48 + more) / 128.0;
    let first: Vec<f32> = (0..48).map(spread(7, 0)).collect();
    let second: Vec<f32> = (0..48).map(spread(11, 48)).collect();
    let mut answers = vec![second.clone(), first.clone()];
    let mut shelf = Shelf::default();
    // The halves are of one channel and of two: another split than SynthSeg's.
    let standard = shelf.put(Role::Standard, move |_| {
        let chances = answers.pop().expect("the network is run twice");
        Ok(halves(GRID, 3, 1, &chances))
    });
    shelf.put(Role::Scores, no_scores);
    // An image that differs at every voxel.
    let values: Vec<f32> = (0..16_u8).map(|place| f32::from(place) / 16.0).collect();
    let seen = image(GRID, values.clone());
    let (found, _) = run(&seen, &three(), wanted(false, false), &mut shelf);
    let found = found.expect("the image is labelled");

    // The mirror the second run is given: x counted from its far end, y and z left alone.
    let voxels = (0..16).map(|place| (place / 4, place / 2 % 2, place % 2));
    let mirror: Vec<f32> = voxels
        .map(|(x, y, z)| values[((3 - x) * 2 + y) * 2 + z])
        .collect();
    assert_eq!(standard.runs(), vec![vec![seen], vec![image(GRID, mirror)]]);
    // The rule for the answers, one number at a time: channels 1 and 2 change places, x is
    // counted from its far end, and y and z are left alone.
    let flip = [0, 2, 1];
    let at = |x: usize, y: usize, z: usize, channel: usize| ((x * 2 + y) * 2 + z) * 3 + channel;
    let mut expected = Vec::new();
    for (x, y, z, channel) in
        (0..48).map(|place| (place / 12, place / 6 % 2, place / 3 % 2, place % 3))
    {
        let mirrored = second[at(3 - x, y, z, flip[channel])];
        expected.push(f32::midpoint(first[at(x, y, z, channel)], mirrored));
    }
    assert_eq!(found.probabilities.shape, [1, 4, 2, 2, 3]);
    assert_eq!(floats(&found.probabilities), expected);
}

#[test]
fn the_robust_networks_run_one_after_another_each_on_what_the_last_gave() {
    let seen = image(
        GRID,
        (0..16_u8).map(|place| f32::from(place) / 16.0).collect(),
    );
    let classes = map((0..16).map(|place| place % 2).collect());
    let cleaned = map((0..16).map(|place| place % 3).collect());
    let chances: Vec<f32> = (0..48_u8).map(|place| f32::from(place) / 64.0).collect();
    let mut shelf = Shelf::default();
    let (first_gives, second_gives, third_gives) =
        (classes.clone(), cleaned.clone(), chances.clone());
    let first = shelf.put(Role::Robust1, move |_| Ok(vec![first_gives.clone()]));
    let second = shelf.put(Role::Robust2, move |_| Ok(vec![second_gives.clone()]));
    let third = shelf.put(Role::Robust3, move |_| Ok(halves(GRID, 3, 2, &third_gives)));
    shelf.put(Role::Scores, no_scores);
    let (found, stages) = run(&seen, &three(), wanted(true, false), &mut shelf);
    let found = found.expect("the image is labelled");

    assert_eq!(first.runs(), vec![vec![seen.clone()]]);
    assert_eq!(second.runs(), vec![vec![classes]]);
    assert_eq!(third.runs(), vec![vec![seen, cleaned]]);
    let steps = [1, 2, 3].map(|step| Finding { step, of: 3 });
    assert_eq!(stages, [steps[0], steps[1], steps[2], Checking]);
    let robust = [Role::Robust1, Role::Robust2, Role::Robust3].map(|role| (role, GRID));
    let scores = (Role::Scores, [BOX; 3]);
    assert_eq!(shelf.opened, [robust[0], robust[1], robust[2], scores]);
    // The two halves side by side, and nothing else done to them.
    let expected = Tensor::f32(vec![1, 4, 2, 2, 3], chances).expect("whole");
    assert_eq!(found.probabilities, expected);
    assert_eq!(found.areas, None);
}

#[test]
fn each_channel_of_the_answer_on_the_mirror_comes_from_the_place_the_swap_names() {
    // Three structures going round. This swap is not its own undoing, so it shows which way
    // it is read.
    let lists = lists(&[0, 3, 42], &[1, 2, 0]);
    // On the image the stand-in answers with nothing. On the mirror it answers a quarter, a
    // half and three quarters at every voxel.
    let mut answers = vec![[0.25, 0.5, 0.75].repeat(16), vec![0.0; 48]];
    let mut shelf = Shelf::default();
    shelf.put(Role::Standard, move |_| {
        let chances = answers.pop().expect("the network is run twice");
        Ok(halves(GRID, 3, 2, &chances))
    });
    shelf.put(Role::Scores, no_scores);
    let (found, _) = run(&plain_image(), &lists, wanted(false, false), &mut shelf);
    let found = found.expect("the image is labelled");

    // Channel 0 is the answer's channel 1, channel 1 is its channel 2 and channel 2 is its
    // channel 0. Each is then halved with the nothing of the first answer. Read the other
    // way round, the three would be 0.375, 0.125 and 0.25.
    let expected = [0.25, 0.375, 0.125].repeat(16);
    assert_eq!(floats(&found.probabilities), expected);
}
