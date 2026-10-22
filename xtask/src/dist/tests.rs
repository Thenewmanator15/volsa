//! The package, made from made-up files in temporary folders: no program is built.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use serde_json::json;
use zip::ZipArchive;

use super::zipped::{civil, zip_time};
use super::{Asked, DOCUMENTS, Sources, asked, assemble, version_and_target};

/// A made-up repository with every document the package carries, a made-up program, and a
/// made-up runtime folder with its two libraries, its licence and notices, and DirectML.
struct Made {
    root: tempfile::TempDir,
}

impl Made {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("a temporary folder");
        for (_, from) in DOCUMENTS {
            write(&root.path().join(from), &format!("the text of {from}"));
        }
        write(&root.path().join("build/voxa.exe"), "a made-up program");
        let runtime = root.path().join("runtime-files");
        for name in [
            "onnxruntime.dll",
            "onnxruntime_providers_shared.dll",
            "LICENSE",
            "ThirdPartyNotices.txt",
            "DirectML.dll",
            "onnxruntime.lib",
        ] {
            write(&runtime.join(name), &format!("the made-up {name}"));
        }
        Self { root }
    }

    fn path(&self, inner: &str) -> std::path::PathBuf {
        self.root.path().join(inner)
    }

    /// Packs version 9.8.7 into `dist`, with the runtime folder if `runtime`, for release.
    fn pack(&self, runtime: bool) -> Result<super::Made, String> {
        self.pack_as(runtime, false)
    }

    /// Packs version 9.8.7 into `dist`, with the runtime folder if `runtime`, and for this
    /// computer alone if `local`.
    fn pack_as(&self, runtime: bool, local: bool) -> Result<super::Made, String> {
        let (program, runtime_files) = (self.path("build/voxa.exe"), self.path("runtime-files"));
        let sources = Sources {
            program: &program,
            root: self.root.path(),
            runtime: runtime.then_some(runtime_files.as_path()),
            local,
        };
        assemble(&sources, "9.8.7", &self.path("dist"))
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a folder")).expect("the folder made");
    fs::write(path, text).expect("written");
}

/// Every entry of a zip, by name, with what it holds.
fn entries(zip: &Path) -> Vec<(String, String)> {
    let mut archive = ZipArchive::new(File::open(zip).expect("the zip")).expect("a zip");
    (0..archive.len())
        .map(|index| {
            let mut entry = archive.by_index(index).expect("an entry");
            let mut text = String::new();
            entry.read_to_string(&mut text).expect("read");
            (entry.name().expect("a name").into_owned(), text)
        })
        .collect()
}

const PROGRAM_AND_DOCUMENTS: [&str; 9] = [
    "voxa.exe",
    "README.txt",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    "NOTICE",
    "THIRD-PARTY-NOTICES.txt",
    "MODELS.md",
    "CHANGELOG.md",
    "INTENDED-USE.txt",
];

#[test]
fn the_package_is_one_folder_with_the_program_its_documents_and_the_runtime() {
    let made = Made::new();
    let packed = made.pack(true).expect("packed");
    assert_eq!(packed.zip, made.path("dist/voxa-9.8.7-windows-x64.zip"));
    assert_eq!(packed.folder, made.path("dist/voxa-9.8.7"));
    let runtime = [
        "runtime/onnxruntime.dll",
        "runtime/onnxruntime_providers_shared.dll",
        "runtime/LICENSE",
        "runtime/ThirdPartyNotices.txt",
    ];
    let expected: Vec<&str> = PROGRAM_AND_DOCUMENTS.into_iter().chain(runtime).collect();
    assert_eq!(packed.files, expected);
    assert!(packed.missing.is_empty(), "{:?}", packed.missing);
    // The zip holds the folder, its runtime folder and every file, each as it came.
    let zipped = entries(&packed.zip);
    let names: Vec<&str> = zipped.iter().map(|(name, _)| name.as_str()).collect();
    let mut wanted = vec!["voxa-9.8.7/", "voxa-9.8.7/runtime/"];
    let files: Vec<String> = expected
        .iter()
        .map(|name| format!("voxa-9.8.7/{name}"))
        .collect();
    wanted.extend(files.iter().map(String::as_str));
    assert_eq!(names, wanted);
    let text = |name: &str| {
        zipped
            .iter()
            .find(|(entry, _)| entry == name)
            .map(|(_, text)| text.clone())
    };
    assert_eq!(
        text("voxa-9.8.7/voxa.exe").as_deref(),
        Some("a made-up program")
    );
    assert_eq!(
        text("voxa-9.8.7/MODELS.md").as_deref(),
        Some("the text of docs/models.md")
    );
    assert_eq!(
        text("voxa-9.8.7/INTENDED-USE.txt").as_deref(),
        Some("the text of docs/intended-use.md")
    );
    assert_eq!(
        text("voxa-9.8.7/runtime/onnxruntime.dll").as_deref(),
        Some("the made-up onnxruntime.dll")
    );
    // The folder beside the zip holds the same files, and DirectML is in neither.
    for name in &expected {
        assert!(packed.folder.join(name).is_file(), "{name} in the folder");
    }
    assert!(!packed.folder.join("runtime/DirectML.dll").exists());
    assert!(
        !names
            .iter()
            .any(|name| name.contains("DirectML") || name.contains("onnxruntime.lib"))
    );
}

#[test]
fn without_a_runtime_the_package_has_no_runtime_folder() {
    let made = Made::new();
    let packed = made.pack(false).expect("packed");
    assert_eq!(packed.files, PROGRAM_AND_DOCUMENTS);
    let zipped = entries(&packed.zip);
    assert_eq!(
        zipped.len(),
        1 + PROGRAM_AND_DOCUMENTS.len(),
        "the folder and its files"
    );
    assert!(!zipped.iter().any(|(name, _)| name.contains("runtime/")));
    assert!(!packed.folder.join("runtime").exists());
}

#[test]
fn a_runtime_folder_without_its_licence_or_notices_is_refused_for_a_release() {
    let made = Made::new();
    fs::remove_file(made.path("runtime-files/LICENSE")).expect("removed");
    fs::remove_file(made.path("runtime-files/ThirdPartyNotices.txt")).expect("removed");
    let why = made.pack(true).expect_err("refused");
    let folder = made.path("runtime-files").display().to_string();
    let sentence = format!(
        concat!(
            "{} holds no LICENSE or LICENSE.txt and no ThirdPartyNotices.txt or ",
            "ThirdPartyNotices, ONNX Runtime's own licence and notices, which a release must ",
            "carry with the runtime: put them in the folder, or give --local for a package ",
            "that is not for release"
        ),
        folder
    );
    assert_eq!(why, sentence);
    assert!(!made.path("dist").exists(), "nothing is made");
    // Lacking one of them is enough.
    let made = Made::new();
    fs::remove_file(made.path("runtime-files/ThirdPartyNotices.txt")).expect("removed");
    let why = made.pack(true).expect_err("refused");
    assert!(
        why.contains("holds no ThirdPartyNotices.txt or ThirdPartyNotices,"),
        "{why}"
    );
}

#[test]
fn the_runtime_notices_are_found_with_or_without_txt_and_carried_by_one_name() {
    let made = Made::new();
    let runtime = made.path("runtime-files");
    fs::rename(runtime.join("LICENSE"), runtime.join("LICENSE.txt")).expect("renamed");
    let notices = runtime.join("ThirdPartyNotices.txt");
    fs::rename(&notices, runtime.join("ThirdPartyNotices")).expect("renamed");
    let packed = made.pack(true).expect("packed");
    let carried = |name: &str| packed.files.iter().any(|file| file == name);
    assert!(carried("runtime/LICENSE") && carried("runtime/ThirdPartyNotices.txt"));
    let licence = fs::read_to_string(packed.folder.join("runtime/LICENSE")).expect("read");
    assert_eq!(licence, "the made-up LICENSE");
    assert!(packed.missing.is_empty(), "{:?}", packed.missing);
}

#[test]
fn a_local_package_goes_without_them_says_so_and_is_named_not_for_release() {
    let made = Made::new();
    fs::remove_file(made.path("runtime-files/LICENSE")).expect("removed");
    fs::remove_file(made.path("runtime-files/ThirdPartyNotices.txt")).expect("removed");
    let packed = made.pack_as(true, true).expect("packed");
    assert_eq!(
        packed.zip,
        made.path("dist/voxa-9.8.7-windows-x64-local.zip")
    );
    assert!(packed.files.contains(&"runtime/onnxruntime.dll".to_owned()));
    assert!(
        !packed
            .files
            .iter()
            .any(|name| name.starts_with("runtime/LICENSE"))
    );
    let folder = made.path("runtime-files").display().to_string();
    let goes = "so this package, which is not for release, goes without it";
    let expected: Vec<String> = [
        "LICENSE or LICENSE.txt",
        "ThirdPartyNotices.txt or ThirdPartyNotices",
    ]
    .iter()
    .map(|named| format!("{folder} holds no {named}, ONNX Runtime's own, {goes}"))
    .collect();
    assert_eq!(packed.missing, expected);
    // A local package with everything in it is still named as one.
    let made = Made::new();
    let packed = made.pack_as(true, true).expect("packed");
    assert!(packed.missing.is_empty(), "{:?}", packed.missing);
    assert_eq!(
        packed.zip,
        made.path("dist/voxa-9.8.7-windows-x64-local.zip")
    );
}

#[test]
fn a_file_the_package_must_carry_that_is_not_there_stops_it_and_is_named() {
    let made = Made::new();
    fs::remove_file(made.path("docs/intended-use.md")).expect("removed");
    let why = made.pack(false).expect_err("stopped");
    let missing = made.path("docs/intended-use.md").display().to_string();
    assert_eq!(
        why,
        format!("the package needs {missing} as INTENDED-USE.txt, and there is no such file")
    );
    assert!(!made.path("dist").exists(), "nothing is made");
    // A runtime folder without one of its libraries stops it too.
    let made = Made::new();
    fs::remove_file(made.path("runtime-files/onnxruntime_providers_shared.dll")).expect("removed");
    let why = made.pack(true).expect_err("stopped");
    assert!(
        why.ends_with("as runtime/onnxruntime_providers_shared.dll, and there is no such file"),
        "{why}"
    );
}

#[test]
fn packing_again_replaces_what_was_packed_before() {
    let made = Made::new();
    made.pack(true).expect("packed");
    write(&made.path("dist/voxa-9.8.7/left over.txt"), "from before");
    let packed = made.pack(false).expect("packed again");
    assert!(!packed.folder.join("left over.txt").exists());
    assert!(!packed.folder.join("runtime").exists());
    assert_eq!(entries(&packed.zip).len(), 1 + PROGRAM_AND_DOCUMENTS.len());
}

#[test]
fn the_version_and_the_target_folder_are_what_cargo_metadata_says() {
    let said = json!({
        "packages": [
            {"name": "voxa-core", "version": "1.0.0"},
            {"name": "voxa-app", "version": "0.5.0"},
        ],
        "target_directory": "C:\\made\\up\\target",
    });
    let (version, target) = version_and_target(&said).expect("both");
    assert_eq!(version, "0.5.0");
    assert_eq!(target, Path::new("C:\\made\\up\\target"));
    let without =
        json!({"packages": [{"name": "voxa-core", "version": "1.0.0"}], "target_directory": "t"});
    assert_eq!(version_and_target(&without), None);
}

#[test]
fn the_package_is_asked_for_with_a_runtime_folder_and_for_this_computer_alone() {
    let args = |words: &[&str]| words.iter().map(ToString::to_string).collect::<Vec<_>>();
    let runtime = |folder: &str| Some(std::path::PathBuf::from(folder));
    assert_eq!(asked(&[]), Ok(Asked::default()));
    let named = asked(&args(&["--runtime", "some folder"]));
    let expected = Asked {
        runtime: runtime("some folder"),
        local: false,
    };
    assert_eq!(named, Ok(expected));
    let local = Asked {
        runtime: runtime("x"),
        local: true,
    };
    assert_eq!(
        asked(&args(&["--runtime", "x", "--local"])),
        Ok(local.clone())
    );
    assert_eq!(asked(&args(&["--local", "--runtime", "x"])), Ok(local));
    let alone = Asked {
        runtime: None,
        local: true,
    };
    assert_eq!(asked(&args(&["--local"])), Ok(alone));
    let usage = Err("usage: cargo xtask dist [--runtime FOLDER] [--local]".to_owned());
    for wrong in [
        &["--runtime"][..],
        &["runtime", "x"],
        &["--runtime", "x", "y"],
        &["--runtime", "--local"],
        &["--local", "--local"],
        &["--runtime", "x", "--runtime", "y"],
        &["x"],
    ] {
        assert_eq!(asked(&args(wrong)), usage, "{wrong:?}");
    }
}

#[test]
fn a_zip_is_dated_by_the_day_and_time_at_greenwich() {
    assert_eq!(civil(0), (1970, 1, 1));
    assert_eq!(civil(10_957), (2000, 1, 1));
    assert_eq!(civil(11_016), (2000, 2, 29), "2000 has a leap day");
    assert_eq!(civil(47_117), (2099, 1, 1));
    assert_eq!(civil(47_175), (2099, 2, 28));
    assert_eq!(civil(47_176), (2099, 3, 1), "2099 has none");
    let when = zip_time(20_748 * 86_400 + 13 * 3600 + 4 * 60 + 5);
    let parts = (
        when.year(),
        when.month(),
        when.day(),
        when.hour(),
        when.minute(),
        when.second(),
    );
    assert_eq!(
        parts,
        (2026, 10, 22, 13, 4, 4),
        "a zip keeps every other second"
    );
    // A time a zip cannot hold is the start of 1980.
    assert_eq!(zip_time(0), zip::DateTime::default());
}
