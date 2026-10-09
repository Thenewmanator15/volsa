# Voxa

Voxa is a desktop viewer for MRI scan exports, written in Rust. It opens a folder, a zip or a
`DICOMDIR`, lists the studies and series inside, and shows any series as a stack of slices with
correct orientation, contrast controls, overlays, tag inspection and cine playback.

**Voxa is not a medical device and is not for diagnostic use.**

![Voxa showing a made-up test study: a ball with a bead near the front](docs/images/voxa-phantom.png)

The picture shows a made-up study that Voxa's own example writes, not a scan of anyone.

Status: version 0.1.0, the first milestone (Foundation). What it does and what it does not yet do
are in the [changelog](CHANGELOG.md); how it was checked is in
[docs/acceptance-0.1.0.md](docs/acceptance-0.1.0.md). The design and the plan for every
milestone are in [docs/superpowers](docs/superpowers).

## Build and run

Voxa needs the Rust toolchain named in `rust-toolchain.toml`; `rustup` installs it on first use.

```bash
cargo run --release -p voxa-app -- path/to/export.zip
```

## Check your work

```bash
cargo xtask check
```

runs, in order: formatting, lints, the 400-line limit on source files, every test, licence and
advisory checks (`cargo deny`), and the documentation build. It stops at the first failure.
`cargo deny` is installed with `cargo install --locked cargo-deny`.

```bash
cargo xtask coverage
```

measures line coverage of `voxa-core`, `voxa-dicom` and `voxa-volume` and fails below 80% on any
of them. It needs `cargo install --locked cargo-llvm-cov`.

## Layout

| Crate | Purpose |
|---|---|
| `voxa-core` | Data model, typed coordinate spaces, errors |
| `voxa-dicom` | Sources (folder, zip, DICOMDIR), header indexing, pixel decoding |
| `voxa-volume` | Volumes from series, sampling and lines of sight, histograms and automatic window/level |
| `voxa-render` | GPU drawing of slices and of volumes; the 3D camera |
| `voxa-app` | The application: interface, state, background work |
| `xtask` | Project tasks (`check`, `coverage`) |

Crates depend only downward: core, then dicom and volume, then render, then app. See
[docs/architecture.md](docs/architecture.md) and the decision records in [docs/adr](docs/adr).

## Privacy

Voxa makes no network connections and collects nothing. Patient names and identifiers are hidden
by default in the interface and are never written to logs. Scan data never belongs in this
repository: tests build synthetic images.

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Contributions are accepted under the same terms.

Voxa is built on other people's libraries, each under its own licence; `cargo deny check` holds
them to the permissive licences listed in [deny.toml](deny.toml). This software is based in part
on the work of the Independent JPEG Group.

The typefaces in the interface come with the `egui` library: Ubuntu Light (Ubuntu Font Licence
1.0), Hack (MIT) and Noto Emoji (SIL Open Font Licence 1.1). No other font files are in this
repository.

## Trying it without a scan

```
cargo run -p voxa-app --example write_phantom -- phantom
cargo run -p voxa-app -- phantom
```

The first command writes a small made-up study (a ball, brighter towards the patient's left, with
a bead near the front) to the folder `phantom`; the second opens it. Setting the environment
variable `VOXA_CAPTURE` to a `.png` path makes Voxa save one picture of its own window and close,
which is how the pictures in the documentation are made.
