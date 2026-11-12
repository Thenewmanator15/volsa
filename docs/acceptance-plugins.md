# Acceptance record for plugins

Date: 31 October 2026. Machine: a Windows 11 desktop with 31.7 GB of memory, a 12th Gen Intel
Core i7-12700K (20 threads), an NVIDIA GeForce RTX 5070 Ti (16 GB) and Intel UHD Graphics 770.
Runtime: ONNX Runtime 1.24.4 with DirectML. Build: `--release`, Rust 1.99.0. Branch:
`claude/plugins-4d9099`.

This record says what was checked of plugins with a real model someone else made, how, and with
what result. Everything run here is the made-up head that the SynthSeg converter paints from
SynthSeg's sample label map, which is nobody's scan: no real scan was used, and nothing in this
record describes a patient. **Nothing derived from the model's weights is in this repository**:
the downloaded files, the converter and its output live in a git-ignored folder outside it
(`dicom-analysis\models\`). The test that runs a plugin names its folder by an environment
variable and holds no part of it.

## The model

MindGrab, Brainchop's skull strip ("omnimodal Skull Strip"), as its browser model: `model.json`
(68 331 bytes) and `model.bin` (584 948 bytes) from `public/models/mindgrab` on the `master`
branch of `github.com/neuroneural/brainchop`, downloaded on 31 October 2026 with the owner's
consent, their sizes checked before use. The Brainchop repository is MIT; a licence for the
weights themselves is not stated, and the local `plugin.json` says so. Voxa recommends no
plugin, and this one was used only to check Voxa.

A short Python script, run with the analysis environment already on this computer (`-I`, paths
as arguments, nothing else downloaded), read the browser model's layer list and weights and
wrote an ONNX network (opset 17) and a `plugin.json`. What it found:

- **The layer list is not the whole network.** It holds 25 dilated 3 × 3 × 3 convolutions
  without bias (dilations 16, 8, 4, 2, 1, five times over, 15 channels), each followed by GELU,
  then a 1 × 1 × 1 convolution with bias to 2 channels. Brainchop's own inference loop
  (`inference-logic.js`) also normalises each channel over the whole volume after every layer
  whose name ends in `_gn` (`LayerNormInPlace` in `tensor-utils.js`: mean and variance over the
  spatial axes, epsilon 1e-5), which the file does not show. Converted without it, the network's
  scores grew to about 10¹⁸ and it marked the whole scanned block; with it as an
  `InstanceNormalization` of no scale or shift, it marks the brain.
- **Preparation, from Brainchop's source.** Brainchop conforms a scan to 1 mm, 256 cubed, in
  FreeSurfer's LIA order with eight-bit values; scales it from its lowest to its highest value
  (quantile scaling is off for MindGrab); and transposes the tensor so that its axes are the
  volume's own, LIA. The `plugin.json` says: `spacing_mm` 1, 1, 1; `axes` `LIA`; one whole box
  of 256 cubed, `outside` `lowest`; percentiles 0 and 100 over the grid; scores of 2 channels,
  the largest piece kept. Voxa does not copy the eight-bit step, which the plugin's limits say,
  nor Brainchop's crop to the voxels above half the maximum with a 20-voxel margin, which only
  narrows what the network sees.

The plan for this check named percentiles 2 and 98; Brainchop's source showed min-max, and the
source was followed.

## What Voxa did with it

`crates/voxa-plugins/tests/with_plugin.rs::a_plugin_named_by_voxa_test_plugin_masks_the_made_up_head`,
with `VOXA_TEST_PLUGIN` naming the converted folder: the folder passed every check of a folder
offered (description, file sizes and sums, the network's structure, the network fitting the
description), was copied as Voxa copies one and checked again, was measured on each device, and
ran through the helper program and ONNX Runtime on the made-up head (144 × 160 × 156 voxels of
1 mm). Its mask was read back onto the head's own grid and held against the brain SynthSeg finds
in it.

| Where | Dice with the brain and the fluid around it | Dice with the brain alone | Share of the brain covered | Run | Helper's ordinary memory, measured |
|---|---|---|---|---|---|
| the processor | 0.978 | 0.848 | 0.998 | 98 to 141 s over three runs | 2 006 MiB |
| RTX 5070 Ti | 0.978 | 0.848 | 0.998 | 2.4 to 2.5 s over three runs | 2 933 to 2 938 MiB |

Windows lists the RTX 5070 Ti twice on this computer, under one identity. Adding a plugin
measured it twice, and its entry then said it runs on that card twice; that was fixed (a card
is measured once), and the test runs on each card once. Run outside
Voxa, directly in ONNX Runtime from Python, the converted network gave the same figures (Dice
0.848 with the brain alone, covering 0.998), so Voxa's preparation and its mapping back to the
scan add nothing. With Brainchop's crop in place of Voxa's whole box, also outside Voxa, it gave
0.979 and 0.836: the crop is not what Voxa's figures miss.

**The mark.** The plan set a Dice score of at least 0.85 with the brain as every SynthSeg label
but 0 and 24, label 24 being the fluid around the brain, and coverage of at least 0.90. MindGrab's
mask keeps that fluid, as a skull strip's mask does, and scored 0.848 by that definition. The
test now asks for a Dice score of at least 0.85 with the brain and its fluid (every label but 0)
and coverage of at least 0.90 of the brain without it, and prints the score with the brain alone
too. The owner was told and, on 31 October 2026, kept this definition. The mark is functional, not
voxel-for-voxel: it shows the mask covers the made-up brain and leaves out what lies outside it.

On the processor the network itself is slow on this computer: ONNX Runtime alone takes 100 to
110 s for it there. The same network converted without its normalisation steps ran in about
15 s in Voxa, so normalising each channel over 256 cubed, 25 times, takes most of the time. On
the card the run takes 2.5 s.

The plugin was not added through the window by hand for this record: the card and the warning
were looked at in pictures drawn from made-up state (`VOXA_CAPTURE_SHOW=plugin-card`,
`plugin-warning`, `plugins` with `parts`, at 900 × 600 and 640 × 480), and the real plugin went
through the same steps the window's **Add** and **Run** take, from the test.

## Budgets (design, section 15)

Measured in a release build, from `crates/voxa-plugins/tests/budgets.rs` and
`with_plugin.rs::preparing_the_plugins_whole_box_from_the_made_up_head_takes_at_most_2_s`.

| Budget | Limit | Measured |
|---|---|---|
| Checking a plugin whose network is 100 MB (a test network with a 100 MB tensor) | 2 s | 0.07 s |
| A run's start-up: helper started and runtime loaded, beyond opening the network | 1.5 s | 0.06 to 0.07 s with `voxa.exe --plugin-helper`, three runs; 0.08 s with the test helper program |
| Preparing MindGrab's box of 256 cubed from the made-up head, on the processor | 2 s | 0.13 s |

## Not checked

- Adding and running a plugin by hand in the window, and how a person reads the card and the
  warning; this goes into the usability file when the owner first tries it.
- A plugin in blocks, with a real model (blocks were checked with test networks only).
- Any real scan.
- Linux and macOS, which were neither built nor run.

## After the merge onto Volsa (12 November 2026)

The plugins work was brought onto `main` as it stood on 12 November 2026 (the name Volsa, the
new window, volumes held as stored), and checked again on the same machine with the same
runtime and the same converted MindGrab folder. No real scan was used.

- The four tests of `crates/volsa-plugins/tests/with_runtime.rs` pass, in a release build.
- `crates/volsa-plugins/tests/with_plugin.rs::a_plugin_named_by_volsa_test_plugin_masks_the_made_up_head`
  passes: on the processor, Dice 0.978 with the brain and its fluid, 0.848 with the brain
  alone, 0.998 of the brain covered, run 26.0 s, helper's measured peak 2006 MiB; on the
  NVIDIA GeForce RTX 5070 Ti, the same three figures, run 2.5 s, peak 2963 MiB. The Dice
  figures are those of 31 October.
- `crates/volsa-app/src/worker/plugins_run_tests.rs::a_real_plugin_is_added_and_run_through_volsa_itself`
  passes: the app's own workers offered, added and measured the folder and ran it on the
  processor through `volsa.exe --plugin-helper`, and its labels and table were kept.
- `crates/volsa-app/src/ui/plugins/pressed_tests.rs::a_plugin_is_added_run_and_removed_by_pressing_the_buttons_in_order`
  presses the buttons of the Tools tab and of the card, the warning and the question, in order,
  on the real state and workers with a stand-in helper.
- Pictures of the window with a made-up plugin (`VOLSA_CAPTURE_SHOW` `tab-tools,plugins` and
  `tab-tools,plugin-card`) were looked at: the entry is under "Added by you" in the Tools tab,
  and the card says Volsa's words before the plugin's.

Not done: the buttons pressed by a person's own hand in the window, and the system's window for
choosing a folder.
