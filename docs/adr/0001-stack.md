# 1. Rust with egui and wgpu

> Volsa was earlier called Voxa. This record keeps the name it was written with.

Date: 2026-10-05. Status: accepted.

## Context

Voxa must open large scans quickly, draw them at 60 frames a second, run on Windows first and
Linux and macOS later, and stay maintainable by a small number of people. Later milestones need
GPU volume rendering and on-device model inference.

## Decision

Rust for the whole application. `egui` (through `eframe`) for the interface and `wgpu` for
drawing, using the `wgpu` that `egui-wgpu` re-exports so there is exactly one GPU device.
`dicom-rs` for reading DICOM. No web view and no second language.

## Consequences

- One toolchain, one binary, no runtime to install.
- `egui`'s text and layout are plainer than a web interface: the theme, fonts and spacing are set
  deliberately in one place from the first milestone.
- Methods that exist only as Python (registration, bias correction, model inference) must be
  rewritten or converted; each is validated against the reference implementation's output.
- `wgpu` and `egui` release often and break interfaces. Versions are pinned in the workspace
  manifest and upgraded together, in a commit of their own.
