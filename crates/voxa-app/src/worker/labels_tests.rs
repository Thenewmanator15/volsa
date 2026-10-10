//! Reading label files on the workers: the labels, the table found beside them, what is
//! kept, and what is said when a file is wrong.

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use voxa_labels::testkit::{NiftiOptions, gzipped, nifti};
use voxa_labels::{NiftiError, Side};

use super::labels::{NAMES_NOT_READ, table_beside};
use super::tests::Bench;
use super::*;
use crate::labels::NamesFrom;
use crate::state::NoticeKind;

const TABLE: &str = r##"{"title": "A made-up head", "model": "robust", "regions": {
 "hippocampus": {"group": "Deep brain", "colour": "#dcd814", "ids": {"left": 17, "right": 53}}}}"##;

/// A label volume `side` voxels each way, 2 mm apart: its first half 17, the rest 53.
fn volume(side: u16) -> Vec<u8> {
    let count = u32::from(side).pow(3);
    let options = NiftiOptions {
        dims: [side; 3],
        ids: (0..count)
            .map(|at| if at < count / 2 { 17 } else { 53 })
            .collect(),
        kind: 512,
        srow: Some([
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 2.0, 0.0],
        ]),
        quatern: None,
        big_endian: false,
        slope: 1.0,
    };
    gzipped(&nifti(&options))
}

/// A folder holding these files.
fn folder(files: &[(&str, &[u8])]) -> TempDir {
    let folder = tempfile::tempdir().expect("a temporary folder");
    for (name, bytes) in files {
        std::fs::write(folder.path().join(name), bytes).expect("writes");
    }
    folder
}

fn read(bench: &Bench, labels: PathBuf) {
    let ask = Effect::ReadLabels {
        labels,
        table: None,
    };
    bench.workers.run(ask, None);
}

#[test]
fn labels_and_the_table_beside_them_are_read_kept_and_reported() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder(&[
        ("labels_a.nii.gz", &volume(4)),
        ("regions_a.json", TABLE.as_bytes()),
    ]);
    read(&bench, folder.path().join("labels_a.nii.gz"));
    let Action::LabelsRead {
        title,
        parts,
        verdict,
        corners,
        names_from,
    } = bench.next()
    else {
        panic!("the labels are reported");
    };
    assert_eq!(
        (title.as_str(), names_from, verdict),
        ("A made-up head", NamesFrom::Table, None)
    );
    let said: Vec<_> = parts
        .iter()
        .map(|part| (part.name.as_str(), part.side, part.voxels))
        .collect();
    assert_eq!(
        said,
        [
            ("hippocampus", Side::Left, 32),
            ("hippocampus", Side::Right, 32)
        ]
    );
    assert!(
        (parts[0].ml - 0.256).abs() < 1.0e-9,
        "32 voxels of 8 cubic millimetres"
    );
    let kept = bench.workers.labels().expect("kept");
    assert_eq!(kept.dims, [4, 4, 4]);
    assert_eq!(corners, kept.corners());
    assert!(bench.is_quiet(), "and it is said once");
}

#[test]
fn the_table_is_looked_for_beside_the_labels_in_a_fixed_order() {
    let tried = std::cell::RefCell::new(Vec::new());
    let none = |path: &Path| {
        let name = path
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        tried.borrow_mut().push(name);
        false
    };
    assert_eq!(
        table_beside(Path::new("some/where/labels_a.nii.gz"), none),
        None
    );
    let expected = [
        "labels_a.json",
        "labels_a.ctbl",
        "regions_a.json",
        "regions.json",
        "labels.ctbl",
    ];
    assert_eq!(*tried.borrow(), expected);
    let found = table_beside(Path::new("some/where/parts.NII"), |path| {
        path.ends_with("regions.json")
    });
    assert_eq!(found, Some(PathBuf::from("some/where/regions.json")));
    let own = table_beside(Path::new("x/labels.nii"), |path| {
        path.ends_with("labels.ctbl")
    });
    assert_eq!(own, Some(PathBuf::from("x/labels.ctbl")));
}

#[test]
fn a_file_that_is_not_labels_says_so_and_one_that_is_missing_does_not_give_its_path() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder(&[("noise.nii", &[7; 400])]);
    read(&bench, folder.path().join("noise.nii"));
    assert_eq!(
        bench.next(),
        Action::LabelsFailed(NiftiError::NotNifti.to_string())
    );
    assert!(bench.workers.labels().is_none());
    read(&bench, folder.path().join("secret-name.nii.gz"));
    let Action::LabelsFailed(why) = bench.next() else {
        panic!("it fails");
    };
    assert!(why.ends_with('.') && !why.contains("secret-name"), "{why}");
    let here = folder.path().to_string_lossy().into_owned();
    assert!(!why.contains(&here), "{why}");
}

#[test]
fn a_table_that_cannot_be_read_leaves_the_parts_unnamed_and_says_so() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder(&[("labels_b.nii.gz", &volume(4)), ("labels_b.json", b"noise")]);
    read(&bench, folder.path().join("labels_b.nii.gz"));
    // Everything reported up to the labels themselves, however long the reading takes.
    let mut reports = Vec::new();
    while !matches!(reports.last(), Some(Action::LabelsRead { .. })) {
        reports.push(bench.next());
    }
    let noticed = reports.iter().find_map(|report| match report {
        Action::Post(notice) => Some(notice),
        _ => None,
    });
    let notice = noticed.expect("a notice about the names");
    assert_eq!(
        (notice.kind, notice.text.as_str()),
        (NoticeKind::Warning, NAMES_NOT_READ)
    );
    let read = reports.iter().find_map(|report| match report {
        Action::LabelsRead {
            parts, names_from, ..
        } => Some((parts, names_from)),
        _ => None,
    });
    let (parts, names_from) = read.expect("the labels are still shown");
    assert_eq!(*names_from, NamesFrom::Nothing);
    let said: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(said, ["Label 17", "Label 53"]);
}

#[test]
fn a_table_that_is_named_is_the_one_read() {
    let bench = Bench::with_budget(1 << 20);
    let names = r#"{"parts": {"17": "one", "53": "other"}}"#;
    let folder = folder(&[
        ("labels_a.nii.gz", &volume(4)),
        ("regions_a.json", TABLE.as_bytes()),
        ("mine.json", names.as_bytes()),
    ]);
    let ask = Effect::ReadLabels {
        labels: folder.path().join("labels_a.nii.gz"),
        table: Some(folder.path().join("mine.json")),
    };
    bench.workers.run(ask, None);
    let Action::LabelsRead { parts, .. } = bench.next() else {
        panic!("the labels are reported");
    };
    let said: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(said, ["one", "other"]);
}

#[test]
fn the_newest_read_is_the_one_kept_and_dropping_gives_it_up() {
    let bench = Bench::with_budget(1 << 20);
    let folder = folder(&[("small.nii.gz", &volume(4)), ("large.nii.gz", &volume(6))]);
    read(&bench, folder.path().join("small.nii.gz"));
    read(&bench, folder.path().join("large.nii.gz"));
    // However long the reading takes: until the newer labels are the ones kept.
    let larger = |set: std::sync::Arc<voxa_labels::LabelSet>| set.dims == [6, 6, 6];
    bench.until(|bench| bench.workers.labels().is_some_and(larger));
    let reports = bench.drain();
    let last = reports
        .iter()
        .rev()
        .find(|report| matches!(report, Action::LabelsRead { .. }));
    let Some(Action::LabelsRead { parts, .. }) = last else {
        panic!("the newer labels are reported");
    };
    assert_eq!(
        parts[0].voxels, 108,
        "half of six cubed: the larger volume's"
    );
    assert_eq!(bench.workers.labels().expect("kept").dims, [6, 6, 6]);
    bench.workers.run(Effect::DropLabels, None);
    assert!(bench.workers.labels().is_none());
    // A read that is overtaken by being given up reports nothing and keeps nothing.
    read(&bench, folder.path().join("large.nii.gz"));
    bench.workers.run(Effect::DropLabels, None);
    bench.drain();
    assert!(bench.workers.labels().is_none());
}
