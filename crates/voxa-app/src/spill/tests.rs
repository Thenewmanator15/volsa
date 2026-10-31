use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

#[test]
fn a_slab_put_away_comes_back_the_same() {
    let local = tempfile::tempdir().expect("a temporary folder");
    let back = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&back);
    let spill = SpillFile::create(local.path(), 8)
        .expect("made")
        .when_brought_back(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });
    spill.put_away(1, 0, &[9, 8, 7, 6]).expect("put");
    spill.put_away(1, 4, &[5, 4, 3, 2]).expect("put");
    spill
        .put_away(0, 0, &[1, 2, 3, 4, 5, 6, 7, 8])
        .expect("put");
    let mut slab = [0; 8];
    spill.bring_back(1, 0, &mut slab).expect("back");
    assert_eq!(slab, [9, 8, 7, 6, 5, 4, 3, 2]);
    let mut part = [0; 4];
    spill.bring_back(0, 4, &mut part).expect("back");
    assert_eq!(part, [5, 6, 7, 8]);
    assert_eq!(
        back.load(Ordering::SeqCst),
        0,
        "reading a part says nothing"
    );
    spill.brought_back();
    assert_eq!(
        back.load(Ordering::SeqCst),
        1,
        "a slab back in memory is said"
    );
}

#[test]
fn the_file_holds_the_slabs_and_nothing_else() {
    let local = tempfile::tempdir().expect("a temporary folder");
    let spill = SpillFile::create(local.path(), 4).expect("made");
    spill.put_away(0, 0, &[1, 2, 3, 4]).expect("put");
    spill.put_away(2, 0, &[5, 6]).expect("put");
    // Two slabs of four and a last of two: no header, no name, no tag.
    assert_eq!(spill.len().expect("a length"), 10);
    let name = spill.path().file_name().expect("a name").to_string_lossy();
    assert!(
        name.starts_with("spill-") && name.ends_with(".voxa"),
        "{name}"
    );
}

#[test]
fn the_file_is_deleted_when_its_volume_goes() {
    let local = tempfile::tempdir().expect("a temporary folder");
    let spill = SpillFile::create(local.path(), 4).expect("made");
    spill.put_away(0, 0, &[1, 2, 3, 4]).expect("put");
    let path = spill.path().to_owned();
    assert!(path.exists());
    drop(spill);
    assert!(!path.exists());
}

#[test]
fn leftovers_are_deleted_at_start() {
    let local = tempfile::tempdir().expect("a temporary folder");
    let cache = local.path().join("cache");
    std::fs::create_dir_all(&cache).expect("made");
    std::fs::write(cache.join("spill-0123.voxa"), b"left").expect("written");
    std::fs::write(cache.join("spill-4567.voxa"), b"left").expect("written");
    std::fs::write(cache.join("something else.txt"), b"kept").expect("written");
    assert_eq!(clear_leftovers(local.path()), 2);
    assert!(cache.join("something else.txt").exists());
    assert_eq!(clear_leftovers(&local.path().join("never made")), 0);
}

#[cfg(windows)]
#[test]
fn the_file_cannot_be_opened_by_another_program_while_voxa_has_it() {
    let local = tempfile::tempdir().expect("a temporary folder");
    let spill = SpillFile::create(local.path(), 4).expect("made");
    spill.put_away(0, 0, &[1, 2, 3, 4]).expect("put");
    assert!(File::open(spill.path()).is_err(), "not even to read");
    assert_eq!(
        clear_leftovers(local.path()),
        0,
        "nor deleted while Voxa has it"
    );
}

#[test]
fn a_spill_file_cannot_be_made_where_the_folder_is_a_file() {
    let local = tempfile::tempdir().expect("a temporary folder");
    let not_a_folder = local.path().join("a file");
    std::fs::write(&not_a_folder, b"in the way").expect("written");
    assert!(SpillFile::create(&not_a_folder, 4).is_err());
}
