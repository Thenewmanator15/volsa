<!--
Never put a scan, a scan export, a label file made from a scan, a screenshot that shows
patient details, or tag values in a pull request, its commits or its tests.
-->

## What this changes

<!-- What changes for the person using or building Volsa, in plain words. Link the issue, if there is one. -->

## How it was checked

<!-- The tests that came first, and anything tried by hand. -->

## Checklist

- [ ] Tests first: each new test was seen to fail for the right reason before the code that
      makes it pass was written.
- [ ] `cargo xtask check` passes.
- [ ] No scan data: nothing from a real person's scan (DICOM files, exports, label files,
      screenshots, measurements or tag values) is in the change, its tests or this description.
      Tests build made-up files.
- [ ] The disclaimer is untouched: "Volsa is not a medical device and is not for diagnostic
      use." Nothing in the change claims otherwise.
- [ ] No network access, no telemetry, and no model weights or ONNX Runtime files.
- [ ] Conventional commit messages (`feat:`, `fix:`, `docs:` and so on), one logical change each.
