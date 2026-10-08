# Contributing to Voxa

## Before you start

- Voxa is not a medical device and is not for diagnostic use. Nothing in it may claim otherwise.
- Never commit scan data or anything derived from a real person's scan, including screenshots.
  Tests build synthetic DICOM with `voxa_dicom::testkit`.
- The repository is local for now: do not add a remote or push.

## How work is done

1. Write the test first and watch it fail for the right reason.
2. Write the least code that makes it pass.
3. Run `cargo xtask check` before every commit. It must pass.
4. One logical change per commit, with a conventional message: `feat(dicom): ...`,
   `fix(render): ...`, `docs: ...`, `chore: ...`, `test: ...`, `refactor: ...`.

## Rules the checks enforce

- `cargo fmt`, and `cargo clippy` with pedantic lints and warnings as errors.
- No `unsafe` outside `voxa-render`; any `unsafe` there carries a `// SAFETY:` comment.
- Library code never panics: `unwrap`, `expect`, `panic!` and indexing that can panic are denied
  outside tests. Return a `VoxaError` (or the crate's own `thiserror` enum) instead.
- Every public item has a doc comment.
- Functions stay under 60 lines and source files under 400.
- Dependencies must carry a licence on the allow-list in `deny.toml` and have no open advisory.
  There is exactly one `wgpu` in the tree: the one `egui-wgpu` re-exports.

## Rules the checks cannot enforce

- Crates depend only downward: core, then dicom and volume, then render, then app.
  `voxa-app` contains no image mathematics.
- `Patient`, `Voxel` and `Screen` coordinates are different types. Converting between them takes an
  explicit `Affine`; never pass raw arrays across a crate boundary where a typed point exists.
- Logs go through `tracing` and never contain a patient's name, identifier, birth date or any
  value from DICOM group 0010.
- No network access and no telemetry.
- An automatic result must say how it was produced and how far to trust it, in the interface, not
  only in a log.

## Decisions

Significant choices are recorded in `docs/adr`. Add a record when you make one; do not rewrite an
old record, supersede it.
