use super::unpadded;

#[test]
fn rows_are_copied_without_their_padding() {
    let mut padded = Vec::new();
    for row in [0u8, 10, 20] {
        padded.extend((row..row + 5).chain([9, 9, 9]));
    }
    let picture = unpadded(&padded, 5, 8, 3);
    let wanted: Vec<u8> = [0u8, 10, 20].iter().flat_map(|&row| row..row + 5).collect();
    assert_eq!(picture, wanted);
}

#[test]
fn only_the_whole_rows_held_are_copied() {
    let padded: Vec<u8> = (0..20).collect();
    assert_eq!(
        unpadded(&padded, 5, 8, 3),
        [0, 1, 2, 3, 4, 8, 9, 10, 11, 12]
    );
}
