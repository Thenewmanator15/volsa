//! Tests of what `find` refuses: an image or lists it cannot work with, an answer that is not
//! what a network should give, and a network that says it failed. Whatever a stand-in
//! answers, `find` gives an error and does not panic.

use voxa_models::{Lists, Numbers, Role, RunError, Tensor};

use super::room;
use super::structures;
use super::tests::{
    Answer, BOX, GRID, Shelf, certain, halves, lists, map, no_scores, plain_image, put_top, run,
    three, wanted,
};

const ROBUST: [Role; 3] = [Role::Robust1, Role::Robust2, Role::Robust3];

fn floats_of(shape: &[usize]) -> Tensor {
    Tensor::f32(shape.to_vec(), vec![0.0; shape.iter().product()]).expect("whole")
}

fn bytes_of(shape: &[usize]) -> Tensor {
    Tensor::u8(shape.to_vec(), vec![0; shape.iter().product()]).expect("whole")
}

/// What a network of this role should answer, for the grid and the three structures of
/// these tests.
fn good(role: Role) -> Vec<Tensor> {
    match role {
        Role::Standard | Role::Robust3 => halves(GRID, 3, 2, &certain(&[1; 16], 3)),
        Role::Robust1 | Role::Robust2 | Role::Cortex => vec![map(vec![0; 16])],
        Role::Scores => vec![floats_of(&[1, 4])],
    }
}

/// What `find` gives when every network of a run answers as it should but `role`, which
/// answers with `odd`. The run takes the robust networks when `role` is one of them, and
/// always wants the cortex.
fn with_one_odd(role: Role, odd: impl FnMut(&[Tensor]) -> Answer + Send + 'static) -> Shelf {
    let robust = ROBUST.contains(&role);
    let finding: &[Role] = if robust { &ROBUST } else { &[Role::Standard] };
    let mut shelf = Shelf::default();
    let mut odd = Some(odd);
    for &each in finding.iter().chain(&[Role::Cortex, Role::Scores]) {
        match odd.take_if(|_| each == role) {
            Some(odd) => shelf.put(each, odd),
            None => shelf.put(each, move |_| Ok(good(each))),
        };
    }
    shelf
}

/// The error `find` gives when `role` answers with `odd`.
fn error_from(role: Role, odd: impl FnMut(&[Tensor]) -> Answer + Send + 'static) -> RunError {
    let mut shelf = with_one_odd(role, odd);
    let wanted = wanted(ROBUST.contains(&role), true);
    let (found, _) = run(&plain_image(), &three(), wanted, &mut shelf);
    assert_eq!(
        shelf.alive(),
        0,
        "{role}: no network is kept after a failure"
    );
    found.expect_err("the run fails")
}

/// Holds that an error is a failure, and that its words are a plain sentence that names no
/// file and no place of one.
fn is_a_plain_failure(error: &RunError, about: &str) {
    let RunError::Failed(words) = error else {
        panic!("{about}: {error:?} where a failure was expected");
    };
    assert!(words.ends_with('.'), "{about}: {words}");
    assert!(!words.contains(['/', '\\', ':']), "{about}: {words}");
    assert!(
        !words.contains(".onnx") && !words.contains("model.json"),
        "{about}: {words}"
    );
}

/// Two tensors of floating point numbers, of these shapes.
fn two(first: &[usize], second: &[usize]) -> Vec<Tensor> {
    vec![floats_of(first), floats_of(second)]
}

/// Holds that each of these answers, from the network named beside it, is a plain failure.
fn each_is_a_plain_failure(odd: Vec<(Role, Vec<Tensor>)>) {
    for (role, answer) in odd {
        let about = format!("{role} answering {answer:?}");
        let error = error_from(role, move |_| Ok(answer.clone()));
        is_a_plain_failure(&error, &about);
    }
}

#[test]
fn an_answer_that_is_not_two_halves_of_the_structures_is_a_failure() {
    let (one, two_wide, all) = ([1, 4, 2, 2, 1], [1, 4, 2, 2, 2], [1, 4, 2, 2, 3]);
    each_is_a_plain_failure(vec![
        // The standard network gives two tensors, and so does the third robust one.
        (Role::Standard, vec![]),
        (Role::Standard, vec![floats_of(&all)]),
        (Role::Standard, vec![floats_of(&one); 3]),
        (Role::Robust3, vec![floats_of(&all)]),
        // The two hold three structures between them, and neither holds none.
        (Role::Standard, two(&two_wide, &two_wide)),
        (Role::Standard, two(&one, &one)),
        (Role::Standard, two(&[1, 4, 2, 2, 4], &one)),
        (Role::Standard, two(&all, &[1, 4, 2, 2, 0])),
        (Role::Standard, two(&[1, 4, 2, 2, 0], &all)),
        (Role::Robust3, two(&two_wide, &two_wide)),
        // They are of the image's grid.
        (Role::Standard, two(&[1, 4, 2, 3, 2], &[1, 4, 2, 3, 1])),
        (Role::Standard, two(&two_wide, &[1, 2, 4, 2, 1])),
        (Role::Standard, two(&[4, 2, 2, 2], &[4, 2, 2, 1])),
        (Role::Standard, two(&[], &one)),
        (Role::Robust3, two(&two_wide, &[2, 4, 2, 2, 1])),
        // They are floating point numbers.
        (Role::Standard, vec![bytes_of(&two_wide), floats_of(&one)]),
        (Role::Standard, vec![floats_of(&two_wide), bytes_of(&one)]),
    ]);
}

#[test]
fn an_answer_that_is_not_one_map_or_not_the_scores_is_a_failure() {
    let (whole, with_a_channel) = ([1, 4, 2, 2], [1, 4, 2, 2, 1]);
    each_is_a_plain_failure(vec![
        // A map is one tensor, of a byte for each voxel of the image's grid.
        (Role::Robust1, vec![]),
        (Role::Robust1, vec![floats_of(&whole)]),
        (Role::Robust1, vec![bytes_of(&with_a_channel)]),
        (Role::Robust1, vec![bytes_of(&whole); 2]),
        (Role::Robust2, vec![bytes_of(&[1, 2, 2, 4])]),
        (Role::Robust2, vec![floats_of(&whole)]),
        (Role::Cortex, vec![bytes_of(&[4, 2, 2])]),
        (Role::Cortex, vec![floats_of(&whole)]),
        (Role::Cortex, vec![bytes_of(&whole); 2]),
        // The scores are one tensor: the number that is dropped, and one for each of the
        // three groups.
        (Role::Scores, vec![]),
        (Role::Scores, vec![floats_of(&[1, 3])]),
        (Role::Scores, vec![floats_of(&[1, 5])]),
        (Role::Scores, vec![floats_of(&[4])]),
        (Role::Scores, vec![floats_of(&[1, 4]); 2]),
        (Role::Scores, vec![bytes_of(&[1, 4])]),
    ]);
}

#[test]
fn an_answer_with_other_numbers_than_its_shape_says_is_a_failure() {
    // Put together by hand, a tensor can hold another count than its shape says.
    let short = |numbers: Numbers| Tensor {
        shape: vec![1, 4, 2, 2],
        numbers,
    };
    let chances = Tensor {
        shape: vec![1, 4, 2, 2, 2],
        numbers: Numbers::F32(vec![0.0; 31]),
    };
    let scores = Tensor {
        shape: vec![1, 4],
        numbers: Numbers::F32(vec![0.0; 5]),
    };
    let odd = [
        (Role::Standard, vec![chances, floats_of(&[1, 4, 2, 2, 1])]),
        (Role::Robust1, vec![short(Numbers::U8(vec![0; 15]))]),
        (Role::Cortex, vec![short(Numbers::U8(vec![0; 17]))]),
        (Role::Scores, vec![scores]),
    ];
    for (role, answer) in odd {
        let error = error_from(role, move |_| Ok(answer.clone()));
        is_a_plain_failure(&error, &role.to_string());
    }
}

#[test]
fn an_answer_with_a_number_that_is_not_finite_is_a_failure() {
    // For each: which of the answer's tensors holds the number, and where in it.
    let places = [
        (Role::Standard, 0, 5),
        (Role::Standard, 1, 15),
        (Role::Robust3, 0, 31),
        (Role::Robust3, 1, 0),
        (Role::Scores, 0, 0),
        (Role::Scores, 0, 3),
    ];
    for not_finite in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for (role, tensor, place) in places {
            let mut answer = good(role);
            let Numbers::F32(numbers) = &mut answer[tensor].numbers else {
                panic!("the {role} network gives floating point numbers");
            };
            numbers[place] = not_finite;
            let error = error_from(role, move |_| Ok(answer.clone()));
            is_a_plain_failure(&error, &format!("{role} with {not_finite}"));
        }
    }
}

#[test]
fn a_bad_answer_on_the_mirror_is_a_failure_too() {
    let mut answers = vec![vec![floats_of(&[1, 4, 2, 2, 3])], good(Role::Standard)];
    let error = error_from(Role::Standard, move |_| {
        Ok(answers.pop().expect("two runs"))
    });
    is_a_plain_failure(&error, "the second run of the standard network");
}

#[test]
fn two_answers_too_large_to_add_are_a_failure_and_not_an_infinity() {
    let huge = halves(GRID, 3, 2, &[f32::MAX; 48]);
    let error = error_from(Role::Standard, move |_| Ok(huge.clone()));
    is_a_plain_failure(&error, "a sum past the largest number");
}

#[test]
fn what_a_network_says_went_wrong_is_passed_on_as_it_is() {
    let words = RunError::Failed("the runtime's own words".to_owned());
    for error in [
        RunError::NoRoom,
        RunError::DeviceLost,
        RunError::Stopped,
        words,
    ] {
        for role in Role::ALL {
            let says = error.clone();
            let got = error_from(role, move |_| Err(says.clone()));
            assert_eq!(got, error, "{role}");
        }
    }
}

#[test]
fn a_network_that_cannot_be_opened_is_passed_on_as_it_is() {
    for error in [RunError::NoRoom, RunError::DeviceLost, RunError::Stopped] {
        for role in Role::ALL {
            let mut shelf = with_one_odd(role, move |_| Ok(good(role)));
            shelf.shut = Some((role, error.clone()));
            let wanted = wanted(ROBUST.contains(&role), true);
            let (found, _) = run(&plain_image(), &three(), wanted, &mut shelf);
            assert_eq!(found, Err(error.clone()), "{role}");
            assert_eq!(shelf.opened.last().map(|(last, _)| *last), Some(role));
            assert_eq!(shelf.alive(), 0, "{role}");
        }
    }
}

#[test]
fn what_is_not_an_image_of_one_channel_is_a_failure_before_any_network_is_opened() {
    let odd = [
        ("no channel", floats_of(&[1, 4, 2, 2])),
        ("two channels", floats_of(&[1, 4, 2, 2, 2])),
        ("two images", floats_of(&[2, 4, 2, 2, 1])),
        ("no voxels along x", floats_of(&[1, 0, 2, 2, 1])),
        ("no voxels along z", floats_of(&[1, 4, 2, 0, 1])),
        ("bytes", bytes_of(&[1, 4, 2, 2, 1])),
        (
            "too few numbers",
            Tensor {
                shape: vec![1, 4, 2, 2, 1],
                numbers: Numbers::F32(vec![0.5; 15]),
            },
        ),
        (
            "a size past counting",
            Tensor {
                shape: vec![1, usize::MAX, 2, 2, 1],
                numbers: Numbers::F32(vec![]),
            },
        ),
    ];
    for (about, seen) in odd {
        let mut shelf = Shelf::default();
        let (found, stages) = run(&seen, &three(), wanted(false, true), &mut shelf);
        is_a_plain_failure(&found.expect_err("it is no image"), about);
        assert!(shelf.opened.is_empty() && stages.is_empty(), "{about}");
    }
}

#[test]
fn lists_that_do_not_fit_one_another_are_a_failure_before_any_network_is_opened() {
    let every = |count: usize| (0..count).collect::<Vec<usize>>();
    let labels: Vec<u32> = (0..257).collect();
    let mut odd = vec![
        ("one structure", lists(&[0], &[0])),
        (
            "more structures than a byte can tell apart",
            lists(&labels, &every(257)),
        ),
        ("a swap of too few places", lists(&[0, 3, 42], &[0, 2])),
        (
            "a swap with a place that is not there",
            lists(&[0, 3, 42], &[0, 3, 1]),
        ),
    ];
    let with = |score_class: &[usize], box_side: usize| Lists {
        score_class: score_class.to_vec(),
        box_side,
        ..three()
    };
    odd.push(("too few score classes", with(&[0, 1], BOX)));
    odd.push(("a score class no byte can hold", with(&[0, 256, 2], BOX)));
    odd.push(("a box of nothing", with(&[0, 1, 2], 0)));
    odd.push((
        "a longer box than a model folder may give",
        with(&[0, 1, 2], 513),
    ));
    odd.push(("a box past counting", with(&[0, 1, 2], usize::MAX)));
    for (about, lists) in odd {
        let mut shelf = Shelf::default();
        let (found, stages) = run(&plain_image(), &lists, wanted(true, true), &mut shelf);
        is_a_plain_failure(&found.expect_err("the lists do not fit"), about);
        assert!(shelf.opened.is_empty() && stages.is_empty(), "{about}");
    }
}

#[test]
fn as_many_structures_as_a_byte_can_tell_apart_are_found() {
    // 256 structures, with the last the top one at every voxel.
    let labels: Vec<u32> = (0..256).collect();
    let stay: Vec<usize> = (0..256).collect();
    let mut lists = lists(&labels, &stay);
    lists.score_class = vec![0; 256];
    let mut shelf = Shelf::default();
    put_top(&mut shelf, true, GRID, 256, &[255; 16]);
    let scores = shelf.put(Role::Scores, no_scores);
    let (found, _) = run(&plain_image(), &lists, wanted(true, false), &mut shelf);
    assert_eq!(
        found.expect("the image is labelled").probabilities.shape,
        [1, 4, 2, 2, 256]
    );
    assert_eq!(scores.runs().len(), 1);
}

#[test]
fn the_longest_box_a_model_folder_may_give_is_taken_and_so_is_the_shortest() {
    let with_a_box_of = |box_side: usize| Lists {
        box_side,
        ..three()
    };
    assert_eq!(structures(&with_a_box_of(512)), Ok(3));
    assert_eq!(structures(&with_a_box_of(1)), Ok(3));
    assert!(structures(&with_a_box_of(513)).is_err());
    assert!(structures(&with_a_box_of(0)).is_err());
}

#[test]
fn asking_for_more_memory_than_there_can_be_is_no_room_and_does_not_end_the_program() {
    // More numbers than any machine can hold: each is refused before anything is asked of
    // the system.
    assert_eq!(room::<f32>(usize::MAX / 2).err(), Some(RunError::NoRoom));
    assert_eq!(room::<u8>(usize::MAX).err(), Some(RunError::NoRoom));
    // What can be had is empty, and takes its count without asking for more.
    let mut held = room::<u8>(12).expect("room for twelve");
    assert!(held.is_empty() && held.capacity() >= 12);
    let before = held.as_ptr();
    held.resize(12, 7);
    assert_eq!(held.as_ptr(), before);
}
