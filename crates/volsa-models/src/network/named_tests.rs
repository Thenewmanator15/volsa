//! A network of any plugin: its free sides fixed by the names its file gives them, and its
//! answers counted whatever the width of their numbers.

use super::RunError;

#[test]
fn integers_are_counted_four_and_eight_bytes_wide() {
    use ort::value::TensorElementType;
    assert_eq!(super::width_of(TensorElementType::Float32), Some(4));
    assert_eq!(super::width_of(TensorElementType::Uint8), Some(1));
    assert_eq!(super::width_of(TensorElementType::Int32), Some(4));
    assert_eq!(super::width_of(TensorElementType::Int64), Some(8));
    assert_eq!(super::width_of(TensorElementType::Float16), None);
}

#[test]
fn free_dimensions_are_named_by_the_caller() {
    let free = [("batch".to_owned(), 1), ("side".to_owned(), 256)];
    let overrides = super::overrides(&free).expect("sizes that fit");
    assert_eq!(
        overrides,
        [("batch".to_owned(), 1), ("side".to_owned(), 256)]
    );
    let huge = [("side".to_owned(), usize::MAX)];
    assert!(matches!(super::overrides(&huge), Err(RunError::Failed(_))));
}

#[test]
fn an_output_is_counted_by_the_names_of_its_free_sides() {
    let words = |list: &[&str]| {
        list.iter()
            .map(|word| (*word).to_owned())
            .collect::<Vec<_>>()
    };
    let free = [("side".to_owned(), 64), ("batch".to_owned(), 1)];
    let shape = [-1, 2, -1, -1, -1];
    let names = words(&["batch", "", "side", "side", "side"]);
    assert_eq!(
        super::bytes_named(8, &shape, &names, &free),
        Some(8 * 2 * 64 * 64 * 64)
    );
    let unknown = words(&["batch", "", "side", "other", "side"]);
    assert_eq!(super::bytes_named(8, &shape, &unknown, &free), None);
}
