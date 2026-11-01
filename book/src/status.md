# Status

**Version 0.7.1, the first download.** 0.7.0's release build stopped at its checks, so 0.7.0 was
never offered as a download; 0.7.1 is 0.7.0 with those fixed. It has a new window, made so that
someone who has never seen Volsa can open an export, find the scan and measure on it; it opens
very large scans faster and in less memory; and it refuses a damaged file nested too deeply
instead of stopping.

It also brings out 0.6.0, which was made ready and never published: the download for Windows,
the zip with ONNX Runtime in it for labelling, the notices and credits that go with it, a window
that fits any screen, and fixes found by an audit of what a public release needs. Before that
came 0.5.0, which finished the fourth milestone: 0.4.0 showed labels from files, and 0.5.0
labels a brain scan inside Volsa and reads more kinds of MRI export.

What Volsa does, and what it does not yet do, is in the
[changelog](https://github.com/Thenewmanator15/volsa/blob/main/CHANGELOG.md). How each version
was checked is in its acceptance record:

- the new window: [docs/acceptance-ui-redesign.md](https://github.com/Thenewmanator15/volsa/blob/main/docs/acceptance-ui-redesign.md);
- what large scans cost: [docs/measurements/2026-10-30-large-scans.md](https://github.com/Thenewmanator15/volsa/blob/main/docs/measurements/2026-10-30-large-scans.md);
- labelling: [docs/acceptance-0.5.0.md](https://github.com/Thenewmanator15/volsa/blob/main/docs/acceptance-0.5.0.md);
- more kinds of export: [docs/acceptance-any-mri.md](https://github.com/Thenewmanator15/volsa/blob/main/docs/acceptance-any-mri.md).

The requirements, each traced to the test that shows it is met, the risk register and the
usability file are in [docs/quality](https://github.com/Thenewmanator15/volsa/tree/main/docs/quality).
The design and the plan for every milestone are in
[docs/superpowers](https://github.com/Thenewmanator15/volsa/tree/main/docs/superpowers), and why
labelling is built as it is in
[decision record 0006](https://github.com/Thenewmanator15/volsa/blob/main/docs/adr/0006-voxa-runs-a-labelling-model.md).

Volsa runs on Windows only for now, and has been tried on Windows 11 alone; see
[System requirements](system-requirements.md).
