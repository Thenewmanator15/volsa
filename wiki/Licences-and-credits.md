# Licences and credits

Volsa is licensed under either of the
[Apache License, Version 2.0](https://github.com/Thenewmanator15/volsa/blob/main/LICENSE-APACHE)
or the [MIT license](https://github.com/Thenewmanator15/volsa/blob/main/LICENSE-MIT), at your
option. Contributions are accepted under the same terms.

Volsa is built on other people's libraries, each under its own licence; `cargo deny check` holds
them to the licences listed in
[deny.toml](https://github.com/Thenewmanator15/volsa/blob/main/deny.toml). The licence and notice
texts of every package built into the program are in
[THIRD-PARTY-NOTICES.txt](https://github.com/Thenewmanator15/volsa/blob/main/THIRD-PARTY-NOTICES.txt),
which `cargo xtask notices` writes.
[NOTICE](https://github.com/Thenewmanator15/volsa/blob/main/NOTICE) holds Volsa's own notice and
SynthSeg's attribution. Both come in the zip. This software is based in part on the work of the
Independent JPEG Group. The `volsa.exe` a release offers also has Microsoft's C runtime built
into it, which is under Microsoft's own licence terms, not an open-source licence.

Volsa's labelling steps (`volsa-segment`) are a port to Rust of SynthSeg's own code, Copyright
2020 Benjamin Billot, under the Apache License 2.0. NOTICE carries the attribution and says how
the port differs.

## What labelling uses, which is not in the repository

Each is under its own licence:

- **ONNX Runtime** (Microsoft), which runs the networks, under the MIT licence. Volsa loads it
  from files beside the executable through the `ort` crate (MIT or Apache-2.0); it is not built
  into Volsa. The zip a release offers carries it in its `runtime` folder, with its own licence
  and notices.
- **DirectML** (Microsoft), which lets ONNX Runtime use a graphics card on Windows, under
  Microsoft's own licence: it may be passed on inside an application for use on Windows, it is
  not open source, and its notices must stay with it. Windows 10 from version 1903 and Windows
  11 have a copy of their own, and Volsa carries none.
- **SynthSeg 2.0** (Billot et al.), the model. Its code is Apache 2.0. Its trained networks
  carry no licence statement of their own and are distributed with FreeSurfer, under
  FreeSurfer's terms, so Volsa ships none and fetches none: you get SynthSeg's files under the
  terms they come with and turn them into a model folder on your own computer. FreeSurfer's
  licence says the software is designed for research only and that clinical use is neither
  recommended nor advised; the
  [intended-use page](https://github.com/Thenewmanator15/volsa/blob/main/docs/intended-use.md)
  says what that means for labels Volsa makes.

## Typefaces

The typefaces in the interface come with the `egui` library: Ubuntu Light (Ubuntu Font Licence
1.0), Hack (MIT, and the Bitstream Vera licence for its Bitstream Vera glyphs), Noto Emoji (SIL
Open Font Licence 1.1) and emoji-icon-font (MIT). No other font files are in the repository.
