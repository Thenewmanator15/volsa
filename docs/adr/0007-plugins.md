# 7. Voxa runs models people add themselves, in a helper process

Date: 2026-10-31. Status: accepted. It builds on [record 0006](0006-voxa-runs-a-labelling-model.md),
whose runtime, telemetry and device rules it keeps.

## Context

Voxa runs one model of its own, SynthSeg 2.0, through code written for it. Other models a person
might want (a skull strip, a whole-body segmentation, a measure of age) are published as ONNX
networks with a page of instructions on how to prepare a scan for them. A person who has such a
network should be able to run it in Voxa without Voxa growing code for each model, and without
Voxa vouching for any of them.

A network file is a program of a kind. ONNX Runtime parses it with the person's own rights, and a
fault in that parser, or a network that asks for more memory than the computer has, would take
Voxa down with whatever had not been saved. And a model's answer, wrapped in Voxa's own window,
could be taken for something Voxa made and checked.

The full design is `docs/superpowers/specs/2026-10-31-voxa-plugins-design.md`; the guide for
people who make a plugin is [docs/plugins.md](../plugins.md).

## Decision

- **A plugin is a folder the person adds, described by `plugin.json` (format 1).** Voxa never
  downloads one, recommends none and links to none. Every setting that changes the answer is
  stated in the description; there are no defaults, and a field missing or unknown is refused
  with its name. Only the files the description lists, by plain name, size and SHA-256, are
  copied, into a folder of Voxa's named by the sum of `plugin.json`.
- **Voxa prepares the scan from a fixed menu, not code.** The grid along the scan's own axes
  turned to an axis code, at the plugin's spacing; one centred box or nnU-Net's blocks joined by
  the nearest block's centre; percentiles, z-score, fixed or no scaling; and what lies outside.
  It generalises what `voxa-segment` does for SynthSeg, whose own path and golden tests are
  unchanged. Every input has a fixed size, so the memory a plugin needs is measured once.
- **The network file is read before ONNX Runtime sees it.** A small reader of the protobuf wire
  format (no new dependency) refuses any operator set or node outside the standard ONNX domain,
  ONNX Runtime's own extra operators included; local functions; data outside the listed files;
  and files past its limits (16 levels of subgraphs, 100 000 nodes).
- **Plugin networks run only in a helper process: `voxa.exe --plugin-helper`.** Voxa starts it
  with its pipes, puts it in a Windows job object before telling it anything, and then sends the
  job and the blocks; the helper answers with labels. The job object ends the helper when Voxa
  ends or the job is closed (that is **Stop**), lets it start no process, ends it rather than
  show a crash window, and caps its ordinary memory at the measured need with a quarter added,
  so a run that would pass it fails instead of exhausting the computer. The helper checks the
  network file's SHA-256 itself before it opens it, every run. Scan values pass through the
  pipes only and none is written to disk. The helper loads the runtime as record 0006 says, with
  telemetry off first.
- **Record 0006's "a program holds one runtime for as long as it lives" now holds per process.**
  Voxa's own labelling keeps its runtime for Voxa's life; each helper loads its own and ends with
  its run.
- **`voxa-models/src/process.rs` is the second module of `voxa-models` allowed `unsafe`.** It
  starts the helper in its job object through the system's job-object and process calls; every
  such call carries a `SAFETY` comment. The rest of the crate still denies `unsafe`. The
  `windows` crate gains the features for job objects, process memory counters and threads; no
  new crate is added.
- **A plugin's result is labels, kept as the scan's result for that plugin, with a banner
  first.** "Added by you, not part of Voxa." with the plugin's name, version, maker, source,
  licence and limits, and "Not for diagnostic use.", wherever the labels are shown or written,
  and recorded in the table as `added_by_you`. A scan of a modality the plugin does not list is
  refused; a contrast the description names that the plugin does not list is warned of, run only
  on confirmation and stamped on the result; a description that names none is stamped "Contrast
  not confirmed".
- **One run at a time.** A plugin run and a labelling run share labelling's turn and its **Runs
  on** choice; starting one stops the other.

## Consequences

- Any label-map model that fits the menu runs without code in Voxa. One number, a value map and
  an alignment are part of format 1 already and are refused as "not yet" by this version.
- A model whose preparation the menu cannot express (an eight-bit conform step, for one, as
  Brainchop's MindGrab uses) runs close to its own program but not exactly; a result made in
  blocks can differ from nnU-Net's own near block edges, and says so.
- The helper has the person's own rights: it is not a sandbox. A fault in ONNX Runtime that a
  network file can reach still runs as the person, and SECURITY.md says to add plugins only
  from sources they trust. What the helper process buys is that such a fault, or a run out of
  memory, ends the helper and not Voxa.
- Each run starts a process and loads the runtime again, a fraction of a second against a run of
  seconds.
- Job objects are Windows'. Elsewhere the helper is only killed on stop and when Voxa lets it
  go, and has no memory cap; Linux and macOS have not been built or run, as record 0006 says.
