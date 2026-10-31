//! Where a graphics instance is made. On Windows a driver's OpenGL side can stop a whole run of
//! the tests for good, and an instance that tries every backend is what wakes it, so one place
//! makes the instances of every device started without a window, and leaves OpenGL out there.
//! A source file that makes another, a test's own say, would bring the stoppage back.

use std::path::{Path, PathBuf};

/// Where a graphics instance may be made, as paths from the workspace's folder: the file that
/// starts devices with no window, and the folder of modules it is split into, so that splitting
/// it further does not trip this check. A path ending in a slash is a folder.
const ALLOWED: [&str; 2] = [
    "crates/voxa-render/src/gpu.rs",
    "crates/voxa-render/src/gpu/",
];

/// The ways a graphics instance is made, each as what to look for and a line that makes one
/// that way. Written in pieces so that this file does not hold them.
fn ways() -> Vec<(String, String)> {
    let way = |needle: &[&str], line: &[&str]| (needle.concat(), line.concat());
    vec![
        way(
            &["Instance", "::new("],
            &["let i = wgpu::Instance", "::new(options);"],
        ),
        way(
            &["Instance", "::default("],
            &["let i = wgpu::Instance", "::default();"],
        ),
        // egui-wgpu's own builder: the usual spelling is a call on a setup, after a name.
        way(
            &[".new_", "instance("],
            &["let i = config.wgpu_setup", ".new_", "instance().await;"],
        ),
        way(
            &["new_instance_with_", "webgpu_detection("],
            &[
                "let i = wgpu::util::new_instance_with_",
                "webgpu_detection(d).await;",
            ],
        ),
    ]
}

/// What makes a graphics instance, see [`ways`].
fn needles() -> Vec<String> {
    ways().into_iter().map(|(needle, _)| needle).collect()
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

/// Whether `text` makes an instance with `needle`. A needle that starts like a name must not be
/// the end of a longer one (`AnInstance`); one that starts with a dot is a call on something
/// before it, and what comes before it is that thing's name.
fn holds(text: &str, needle: &str) -> bool {
    let starts_a_name = needle.starts_with(|c: char| c.is_alphanumeric() || c == '_');
    text.match_indices(needle).any(|(at, _)| {
        let before = text.get(..at).and_then(|head| head.chars().next_back());
        !starts_a_name || !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// Whether a graphics instance may be made in the file at `path`, see [`ALLOWED`].
fn is_allowed(path: &str) -> bool {
    ALLOWED.iter().any(|allowed| {
        if allowed.ends_with('/') {
            path.starts_with(allowed)
        } else {
            path == *allowed
        }
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
    let (allowed, strays): (Vec<_>, Vec<_>) = making.into_iter().partition(|path| is_allowed(path));
    assert_eq!(
        strays,
        Vec::<String>::new(),
        "a graphics instance is made only where devices with no window are started (and \
         without OpenGL on Windows): use `voxa_render::render_state_without_a_window` or \
         `HeadlessScene` instead"
    );
    // Where it is allowed must still be where it is made, or the allowance has gone stale.
    assert!(
        !allowed.is_empty(),
        "no allowed file makes an instance any more: {ALLOWED:?}"
    );
}

#[test]
fn the_guard_sees_every_way_of_making_an_instance_and_not_the_names_that_only_end_alike() {
    // A guard that finds nothing anywhere would pass for the wrong reason, and a needle that
    // can never match its own usual spelling is a hole in it.
    for (needle, line) in ways() {
        assert!(
            holds(&line, &needle),
            "`{line}` is not caught by `{needle}`"
        );
    }
    let other_names = [
        ["let i = AnInstance", "::new(options);"].concat(),
        ["let i = renew_instance_with_", "webgpu_detection(d);"].concat(),
    ];
    for line in other_names {
        let caught = needles().iter().any(|needle| holds(&line, needle));
        assert!(!caught, "`{line}` is the end of another name");
    }
}
