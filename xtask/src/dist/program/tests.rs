//! How the program is built for the package and what is read of it, on made-up flags, paths
//! and programs: nothing is built.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::super::pe;
use super::{Remap, cargo_home, flags, remaps, with_manifest, without_places, without_visual_cpp};

/// What separates one encoded flag from the next.
const BETWEEN: &str = "\u{1f}";

#[test]
fn the_cargo_home_is_where_cargo_home_says_or_dot_cargo_in_the_own_folder() {
    let os = |text: &str| Some(OsString::from(text));
    let named = cargo_home(os("D:\\made up\\cargo"), os("C:\\Users\\someone"));
    assert_eq!(named, Some(PathBuf::from("D:\\made up\\cargo")));
    let own = cargo_home(None, os("C:\\Users\\someone"));
    assert_eq!(own, Some(Path::new("C:\\Users\\someone").join(".cargo")));
    assert_eq!(
        cargo_home(os(""), os("/home/someone")),
        Some(PathBuf::from("/home/someone/.cargo"))
    );
    assert_eq!(cargo_home(None, None), None);
}

#[test]
fn the_cargo_home_and_the_workspace_are_taken_out_and_a_target_folder_outside_it() {
    let (home, root) = (
        PathBuf::from("C:\\Users\\someone\\.cargo"),
        Path::new("G:\\voxa"),
    );
    let inside = remaps(Some(home.clone()), root, &root.join("target"));
    let expected: Vec<Remap> = vec![(home.clone(), "cargo"), (root.to_path_buf(), "voxa")];
    assert_eq!(inside, expected);
    let outside = remaps(Some(home.clone()), root, Path::new("E:\\builds"));
    assert_eq!(
        outside.last(),
        Some(&(PathBuf::from("E:\\builds"), "target"))
    );
    assert_eq!(remaps(None, root, &root.join("target")), expected[1..]);
}

#[test]
fn the_flags_add_the_c_runtime_and_the_places_to_any_already_given() {
    let remapped: Vec<Remap> = vec![
        (PathBuf::from("C:\\Users\\some one\\.cargo"), "cargo"),
        (PathBuf::from("G:\\voxa"), "voxa"),
    ];
    let ours = [
        "-Ctarget-feature=+crt-static",
        "--remap-path-prefix=C:\\Users\\some one\\.cargo=cargo",
        "--remap-path-prefix=G:\\voxa=voxa",
    ];
    assert_eq!(flags(None, None, &remapped), ours.join(BETWEEN));
    assert_eq!(flags(Some(""), Some("  "), &remapped), ours.join(BETWEEN));
    // Flags in RUSTFLAGS are split at spaces, as cargo splits them.
    let plain = flags(None, Some("-C debuginfo=1 "), &remapped);
    assert_eq!(
        plain,
        ["-C", "debuginfo=1"]
            .iter()
            .chain(&ours)
            .copied()
            .collect::<Vec<_>>()
            .join(BETWEEN)
    );
    // Encoded flags are kept whole, and taken before RUSTFLAGS, as cargo takes them.
    let given = ["-Cdebuginfo=1", "--cfg=a b"].join(BETWEEN);
    let encoded = flags(Some(&given), Some("-C opt-level=1"), &remapped);
    assert_eq!(
        encoded,
        [given.as_str()]
            .iter()
            .chain(&ours)
            .copied()
            .collect::<Vec<_>>()
            .join(BETWEEN)
    );
}

#[test]
fn a_program_that_still_records_a_place_of_this_computer_is_refused_with_it() {
    let remapped: Vec<Remap> = vec![
        (PathBuf::from("C:\\Users\\someone\\.cargo"), "cargo"),
        (PathBuf::from("G:\\made\\up\\voxa"), "voxa"),
    ];
    let clean = b"MZ... cargo\\registry\\src\\egui-0.36.2\\src\\lib.rs voxa\\crates ...";
    assert_eq!(without_places(clean, &remapped), Ok(()));
    let leaky = b"MZ... C:\\Users\\someone\\.cargo\\registry\\src\\egui\\lib.rs ...";
    assert_eq!(
        without_places(leaky, &remapped),
        Err(concat!(
            "voxa.exe still records C:\\Users\\someone\\.cargo, ",
            "a place on the computer it was built on"
        )
        .to_owned())
    );
    // Written with the other slash, or both places at once.
    let both = b"G:/made/up/voxa/target/out.rs and C:/Users/someone/.cargo/x";
    let said = without_places(both, &remapped).expect_err("refused");
    assert!(
        said.contains("C:\\Users\\someone\\.cargo and G:\\made\\up\\voxa"),
        "{said}"
    );
}

#[test]
fn a_program_without_its_manifest_is_refused() {
    let manifest = concat!(
        "<assemblyIdentity type=\"win32\" name=\"Microsoft.Windows.Common-Controls\"/>",
        "<dpiAwareness>PerMonitorV2, PerMonitor</dpiAwareness>"
    );
    let carrying = format!("MZ... {manifest} ...");
    assert_eq!(with_manifest(carrying.as_bytes()), Ok(()));
    let refused = Err(concat!(
        "voxa.exe carries no manifest asking for the common controls of version 6 and the ",
        "scale of each screen: the build did not put crates/voxa-app/voxa.exe.manifest into it"
    )
    .to_owned());
    assert_eq!(with_manifest(b"MZ... a program with no manifest"), refused);
    let half = b"MZ... <dpiAwareness>PerMonitorV2</dpiAwareness>";
    assert_eq!(with_manifest(half), refused);
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
        Err(concat!(
            "voxa.exe imports VCRUNTIME140.dll, api-ms-win-crt-heap-l1-1-0.dll, so the C ",
            "runtime is not inside it and it would need the Visual C++ redistributable"
        )
        .to_owned())
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
    let names = pe::imports(&std::fs::read(here).expect("read")).expect("a Windows program");
    assert!(
        names
            .iter()
            .any(|name| name.eq_ignore_ascii_case("kernel32.dll")),
        "{names:?}"
    );
}
