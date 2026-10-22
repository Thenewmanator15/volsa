# Contributing to Voxa

Thank you for helping. This file says how to build Voxa, how work is done here, and the rules
every change keeps. To report a fault or ask for something new, use the repository's
[Issues](../../issues); a security vulnerability goes privately, as [SECURITY.md](SECURITY.md)
says.

## Before you start

- Voxa is not a medical device and is not for diagnostic use. Nothing in it may claim otherwise,
  and the sentence that says so stays as it is.
- Never commit scan data or anything derived from a real person's scan, including screenshots,
  label files, measurements and tag values, and never put them in an issue or a pull request.
  Tests build synthetic DICOM with `voxa_dicom::testkit`, and the `write_phantom` example writes
  a made-up study to try Voxa on.
- No model weights and no ONNX Runtime or DirectML files are committed. Voxa ships neither;
  [docs/models.md](docs/models.md) says why.
- Nothing is merged into `main` without its owner's word. For anything larger than a small fix,
  open an issue first and say what you have in mind.

## Building

Install Rust with [rustup](https://rustup.rs). The toolchain is pinned in
`rust-toolchain.toml` (1.99.0, with `rustfmt`, `clippy` and `llvm-tools-preview`) and is
installed the first time you build. Then:

```bash
cargo build
cargo run -p voxa-app --example write_phantom -- phantom
cargo run -p voxa-app -- phantom
```

The second command writes a small made-up study to the folder `phantom` (which git ignores) and
the third opens it. The [README](README.md#building-from-source) says more: labels, the made-up
head, and the tasks below.

The gate needs `cargo deny`, and the coverage task `cargo llvm-cov`:

```bash
cargo install --locked cargo-deny
cargo install --locked cargo-llvm-cov
```

Voxa is built and tried on Windows. The continuous integration also runs the gate on Ubuntu,
where it first installs `libgtk-3-dev`, and `mesa-vulkan-drivers`, a Vulkan driver in software,
so that the drawing tests run without a graphics card.

The tests of `voxa-render` draw with a graphics device. On a computer with two,
`WGPU_POWER_PREF=high` has them use the faster one, and `WGPU_BACKEND` (`dx12` or `vulkan`)
picks the backend. The tests that need the real labelling networks are marked `ignore`;
`cargo xtask check-models`, with `VOXA_MODELS` and `VOXA_RUNTIME` naming a model folder and a
runtime folder, runs them.

## How work is done

1. Write the test first and watch it fail for the right reason.
2. Write the least code that makes it pass.
3. Run `cargo xtask check` before every commit. It must pass.
4. One logical change per commit, with a conventional message: `feat(dicom): ...`,
   `fix(render): ...`, `docs: ...`, `chore: ...`, `test: ...`, `refactor: ...`, `build: ...`,
   `ci: ...`. The subject says in plain words what changed for the person using or building
   Voxa, for example `fix(app): a lost graphics device stops no work and loses nothing`.
5. Open a pull request against `main`. Its template lists what is checked, and the continuous
   integration runs the same gate.

`cargo xtask check` runs, in order and stopping at the first failure: `cargo fmt --check`,
`cargo clippy` on every target and feature with warnings as errors, the 400-line limit on
source files, `cargo test --workspace --all-features`, `cargo deny check`, and the
documentation build with warnings as errors.

## Rules the checks enforce

- `cargo fmt`, and `cargo clippy` with pedantic lints and warnings as errors.
- No `unsafe` outside `voxa-render` and one module of `voxa-models`, `cards.rs` (the system's list
  of graphics cards); any `unsafe` there carries a `// SAFETY:` comment.
- Library code never panics: `unwrap`, `expect`, `panic!` and indexing that can panic are denied
  outside tests. Return a `VoxaError` (or the crate's own `thiserror` enum) instead.
- Every public item has a doc comment.
- Functions stay under 60 lines and source files under 400.
- Dependencies must carry a licence on the allow-list in `deny.toml` and have no open advisory.
  There is exactly one `wgpu` in the tree: the one `egui-wgpu` re-exports.
- `THIRD-PARTY-NOTICES.txt` names every package built into the program, with its licence and
  notice texts. When the dependencies change, run `cargo xtask notices` and commit the file;
  the check fails until it is current.

## Rules the checks cannot enforce

- Crates depend only downward: core, then dicom, volume, labels and models, then segment and
  render, then app. `voxa-app` contains no image mathematics.
- `Patient`, `Voxel` and `Screen` coordinates are different types. Converting between them takes an
  explicit `Affine`; never pass raw arrays across a crate boundary where a typed point exists.
- Logs go through `tracing` and never contain a patient's name, identifier, birth date or any
  value from DICOM group 0010.
- No network access and no telemetry.
- An automatic result must say how it was produced and how far to trust it, in the interface, not
  only in a log.
- Write as the code around you does: doc comments in plain English that say what a thing is and
  what it is for, and names in the same idiom as their neighbours.

## Decisions

Significant choices are recorded in `docs/adr`. Add a record when you make one; do not rewrite an
old record, supersede it.

## Licence

Voxa is licensed under either the MIT licence or the Apache License 2.0, at your option, and
contributions are accepted under the same terms.
