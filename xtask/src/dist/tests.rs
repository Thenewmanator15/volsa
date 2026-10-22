//! The package, made from made-up files in temporary folders: no program is built.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use serde_json::json;
use zip::ZipArchive;

use super::{
    DOCUMENTS, Sources, assemble, civil, pe, runtime_named, version_and_target,
    with_static_runtime, without_visual_cpp, zip_time,
};

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

    /// Packs version 9.8.7 into `dist`, with the runtime folder if `runtime`.
    fn pack(&self, runtime: bool) -> Result<super::Made, String> {
        let (program, runtime_files) = (self.path("build/voxa.exe"), self.path("runtime-files"));
        let sources = Sources {
            program: &program,
            root: self.root.path(),
            runtime: runtime.then_some(runtime_files.as_path()),
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
fn a_runtime_folder_without_its_licence_or_notices_is_packed_and_said_to_lack_them() {
    let made = Made::new();
    fs::remove_file(made.path("runtime-files/LICENSE")).expect("removed");
    fs::remove_file(made.path("runtime-files/ThirdPartyNotices.txt")).expect("removed");
    let packed = made.pack(true).expect("packed all the same");
    assert!(packed.files.contains(&"runtime/onnxruntime.dll".to_owned()));
    assert!(
        !packed
            .files
            .iter()
            .any(|name| name.ends_with("LICENSE") && name.starts_with("runtime"))
    );
    assert_eq!(packed.missing.len(), 2, "{:?}", packed.missing);
    let folder = made.path("runtime-files").display().to_string();
    for (said, name) in packed
        .missing
        .iter()
        .zip(["LICENSE", "ThirdPartyNotices.txt"])
    {
        let sentence = format!(
            "{folder} holds no {name}, ONNX Runtime's own, so the package's runtime folder goes \
             without it; a release must carry it"
        );
        assert_eq!(*said, sentence);
    }
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
fn the_package_is_asked_for_with_a_runtime_folder_or_without_one() {
    let args = |words: &[&str]| words.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(runtime_named(&[]), Ok(None));
    let named = runtime_named(&args(&["--runtime", "some folder"]));
    assert_eq!(named, Ok(Some("some folder".into())));
    let usage = Err("usage: cargo xtask dist [--runtime FOLDER]".to_owned());
    for wrong in [
        &["--runtime"][..],
        &["runtime", "x"],
        &["--runtime", "x", "y"],
        &["x"],
    ] {
        assert_eq!(runtime_named(&args(wrong)), usage, "{wrong:?}");
    }
}

#[test]
fn the_c_runtime_inside_the_program_is_added_to_any_flags_already_given() {
    let flag = "-C target-feature=+crt-static";
    assert_eq!(with_static_runtime(None), flag);
    assert_eq!(with_static_runtime(Some("  ")), flag);
    assert_eq!(
        with_static_runtime(Some("-C debuginfo=1 ")),
        format!("-C debuginfo=1 {flag}")
    );
}

#[test]
fn a_program_that_imports_the_visual_cpp_runtime_is_refused_with_the_names() {
    let names = |words: &[&str]| words.iter().map(ToString::to_string).collect::<Vec<_>>();
    let fine = names(&[
        "KERNEL32.dll",
        "USER32.dll",
        "api-ms-win-core-synch-l1-2-0.dll",
    ]);
    assert_eq!(without_visual_cpp(&fine), Ok(()));
    let needing = names(&[
        "KERNEL32.dll",
        "VCRUNTIME140.dll",
        "api-ms-win-crt-heap-l1-1-0.dll",
    ]);
    assert_eq!(
        without_visual_cpp(&needing),
        Err(
            "voxa.exe imports VCRUNTIME140.dll, api-ms-win-crt-heap-l1-1-0.dll, so the C \
             runtime is not inside it and it would need the Visual C++ redistributable"
                .to_owned()
        )
    );
    for one in ["msvcp140.dll", "ucrtbase.dll", "vcruntime140_1.dll"] {
        assert!(without_visual_cpp(&names(&[one])).is_err(), "{one}");
    }
}

/// A made-up 64-bit program of one section, whose import table names `libraries`.
fn program_importing(libraries: &[&str]) -> Vec<u8> {
    let mut image = vec![0_u8; 0x400];
    let mut put = |at: usize, value: u32, width: usize| {
        image[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]);
    };
    put(0x3c, 0x80, 4);
    put(0x80, u32::from_le_bytes(*b"PE\0\0"), 4);
    let (coff, optional) = (0x84, 0x98);
    put(coff + 2, 1, 2);
    put(coff + 16, 240, 2);
    put(optional, 0x20b, 2);
    put(optional + 108, 16, 4);
    put(optional + 120, 0x1000, 4);
    // The one section: 0x200 bytes from 0x200 in the file, at 0x1000 when loaded.
    let section = optional + 240;
    for (at, value) in [(8, 0x200), (12, 0x1000), (16, 0x200), (20, 0x200)] {
        put(section + at, value, 4);
    }
    let mut name = 0x100;
    for (index, library) in libraries.iter().enumerate() {
        put(
            0x200 + index * 20 + 12,
            0x1000 + u32::try_from(name).expect("small"),
            4,
        );
        name += library.len() + 1;
    }
    let mut name = 0x300;
    for library in libraries {
        image[name..name + library.len()].copy_from_slice(library.as_bytes());
        name += library.len() + 1;
    }
    image
}

#[test]
fn the_import_table_of_a_program_names_the_libraries_it_imports() {
    let image = program_importing(&["KERNEL32.dll", "USER32.dll", "VCRUNTIME140.dll"]);
    let names = pe::imports(&image).expect("read");
    assert_eq!(names, ["KERNEL32.dll", "USER32.dll", "VCRUNTIME140.dll"]);
    assert_eq!(pe::imports(&program_importing(&[])), Ok(Vec::new()));
    // A file that is not a program, or is cut short, cannot be read as one.
    let unreadable = Err("the file cannot be read as a Windows program".to_owned());
    assert_eq!(
        pe::imports(b"a made-up text file, not a program"),
        unreadable
    );
    assert_eq!(pe::imports(&image[..0x210]), unreadable);
    assert_eq!(pe::imports(&[]), unreadable);
}

#[test]
fn a_program_carries_resources_when_its_table_of_resources_is_not_empty() {
    let mut image = program_importing(&["KERNEL32.dll"]);
    assert_eq!(pe::has_resources(&image), Ok(false));
    // The third table in the list: 0x100 bytes at 0x1200.
    image[0x98 + 128..0x98 + 136].copy_from_slice(&[0, 0x12, 0, 0, 0, 1, 0, 0]);
    assert_eq!(pe::has_resources(&image), Ok(true));
    let unreadable = Err("the file cannot be read as a Windows program".to_owned());
    assert_eq!(pe::has_resources(b"a made-up text file"), unreadable);
}

#[cfg(all(windows, target_env = "msvc"))]
#[test]
fn the_import_table_of_this_test_program_names_the_core_of_windows() {
    let here = std::env::current_exe().expect("this program");
    let names = pe::imports(&fs::read(here).expect("read")).expect("a Windows program");
    assert!(
        names
            .iter()
            .any(|name| name.eq_ignore_ascii_case("kernel32.dll")),
        "{names:?}"
    );
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
