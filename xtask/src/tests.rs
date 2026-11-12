//! The tests of the tasks in `main.rs`.

use super::{EVERY_TEST, listed, passed_in, skipped_in, some_listed};

#[test]
fn the_gate_runs_every_test_program_however_many_fail() {
    // Cargo otherwise stops at the first test program that fails, and hides the rest.
    assert!(EVERY_TEST.contains(&"--no-fail-fast"), "{EVERY_TEST:?}");
}

#[test]
fn a_listing_names_its_tests_and_nothing_else() {
    let listing = "network::tests::a_stop_set_beforehand_stops: test\n\
                   the_volume_is_prepared_as_python_prepares_it: test\n\
                   a_made_up_benchmark: bench\n\
                   \n\
                   2 tests, 1 benchmark\n";
    let names = [
        "network::tests::a_stop_set_beforehand_stops",
        "the_volume_is_prepared_as_python_prepares_it",
    ];
    assert_eq!(listed(listing), names);
    // A listing with no test in it names none: empty, or only lines that are not tests.
    let none: Vec<String> = Vec::new();
    assert_eq!(listed(""), none);
    assert_eq!(listed("\n0 tests, 0 benchmarks\n"), none);
    assert_eq!(listed("a name: test and more\n: testing\n"), none);
    // A crate that lists none has not been checked, and that is said, not passed over.
    let said = some_listed("volsa-models", none.clone());
    let why = "volsa-models lists no ignored test, so nothing would have been checked there";
    assert_eq!(said, Err(why.to_owned()));
    let some = vec!["a_made_up_test".to_owned()];
    assert_eq!(some_listed("volsa-models", some.clone()), Ok(some));
}

#[test]
fn a_test_that_skipped_its_work_says_so_in_a_line_of_its_own_words() {
    let why = Some("this computer has no graphics card that is Integrated");
    let alone = "skipped: this computer has no graphics card that is Integrated";
    assert_eq!(skipped_in(alone), why);
    // The first line a test prints follows the test program's own words for the test.
    let first = format!("test the_standard_model_on_integrated_graphics ... {alone}");
    assert_eq!(skipped_in(&first), why);
    let others = [
        "",
        "test a_name ... ok",
        "the standard model on the processor: 40.2 s in all",
        "  against Python: 0 voxels differ; skipped: nothing",
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out",
    ];
    for line in others {
        assert_eq!(skipped_in(line), None, "{line}");
    }
}

#[test]
fn only_the_line_that_sums_up_a_run_says_how_many_tests_passed() {
    let one = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out";
    let none = "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 46 filtered out";
    let failed = "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured";
    let twelve = "test result: ok. 12 passed; 0 failed; 3 ignored; finished in 2.73s";
    let sums = [(one, 1), (none, 0), (failed, 0), (twelve, 12)];
    for (line, passed) in sums {
        assert_eq!(passed_in(line), passed, "{line}");
    }
    // What a test prints, and what stands round it, says nothing of how many passed.
    let others = [
        "",
        "running 1 test",
        "test network::tests::a_stop_set_beforehand_stops ... ok",
        "1 passed; 0 failed",
        "test result: ok. some passed",
        "  against Python: test result: ok. 1 passed",
    ];
    for line in others {
        assert_eq!(passed_in(line), 0, "{line}");
    }
}
