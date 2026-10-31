//! Where a graphics instance is made. On Windows a driver's OpenGL side can stop a whole run of
//! the tests for good, and an instance that tries every backend is what wakes it, so one place
//! makes the instances of every device started without a window, and leaves OpenGL out there.
//! A source file that makes another, a test's own say, would bring the stoppage back.

use std::path::{Path, PathBuf};

/// The one file that makes a graphics instance for a device with no window, from the workspace's
/// folder.
const THE_ONE: &str = "crates/voxa-render/src/gpu.rs";

/// What makes a graphics instance, written in pieces so that this file does not hold it.
fn needles() -> Vec<String> {
    vec![
        ["Instance", "::new("].concat(),
        ["Instance", "::default("].concat(),
        [".new_", "instance("].concat(),
        ["new_instance_with_", "webgpu_detection("].concat(),
    ]
}

/// The workspace's folder: two above this crate's.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate is two folders down in the workspace")
        .to_path_buf()
}

/// Every Rust source under `crates` and `xtask`: sources, tests, benchmarks, examples and build
/// scripts alike.
fn sources(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut folders = vec![root.join("crates"), root.join("xtask")];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).expect("lists a folder") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                folders.push(path);
            } else if path.extension().is_some_and(|ending| ending == "rs") {
                found.push(path);
            }
        }
    }
    found
}

/// Whether `text` makes an instance with `needle`, not as the end of a longer name.
fn holds(text: &str, needle: &str) -> bool {
    text.match_indices(needle).any(|(at, _)| {
        let before = text.get(..at).and_then(|head| head.chars().next_back());
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// The path of `file` from `root`, written with forward slashes.
fn relative(file: &Path, root: &Path) -> String {
    let inside = file.strip_prefix(root).expect("under the workspace");
    inside.to_string_lossy().replace('\\', "/")
}

#[test]
fn only_the_shared_headless_device_makes_a_graphics_instance() {
    let root = workspace();
    let files = sources(&root);
    assert!(
        files.len() > 100,
        "{} sources found under {root:?}",
        files.len()
    );
    let mut making = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("reads a source");
        if needles().iter().any(|needle| holds(&text, needle)) {
            making.push(relative(file, &root));
        }
    }
    making.sort();
    assert_eq!(
        making,
        [THE_ONE],
        "a graphics instance is made only where devices with no window are started (and \
         without OpenGL on Windows): use `voxa_render::render_state_without_a_window` or \
         `HeadlessScene` instead"
    );
}

#[test]
fn the_guard_sees_what_it_guards_against() {
    // A guard that finds nothing anywhere would pass for the wrong reason.
    let made = ["let i = wgpu::Instance", "::new(options);"].concat();
    assert!(needles().iter().any(|needle| holds(&made, needle)));
    let named = ["let i = AnInstance", "::new(options);"].concat();
    assert!(!needles().iter().any(|needle| holds(&named, needle)));
}
