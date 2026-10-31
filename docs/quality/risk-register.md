# Risk register

What could lead someone to be misled or harmed by using Voxa, what keeps that from
happening, and how that is checked. Kept in the spirit of ISO 14971. Voxa is not a medical
device and is not for diagnostic use; the register exists because people will still look
at their own scans with it and draw conclusions.

How bad (severity): **minor** (confusion, wasted time), **serious** (a person believes
something false about their own or someone's body, and may worry, delay or act on it),
**critical** (care is changed because of Voxa). How likely, before the control:
**rare**, **possible**, **likely**. Each entry is reviewed whenever the part of Voxa it names
changes.

Entries from earlier work (labels on the wrong grid, left and right, a failed model check
hidden) are brought in when that work is next changed.

## LS: very large scans (design: 2026-10-30-voxa-large-scans-design.md)

| No. | Hazard and how harm could follow | Severity / likelihood before | Control | Checked by | After |
|---|---|---|---|---|---|
| R-LS-01 | A value held in 16 bits reads differently from the file (sign, slope or intercept mishandled); a measurement or a value under the pointer is wrong. | serious / possible | Exact equality with 0.6.0's decoding for every kind of stored value (LS-02); 32 bits where 16 cannot be exact; the card's copy is for drawing only and measuring always reads the held values. | `crates/voxa-dicom/src/decode/stored_tests.rs`; `crates/voxa-app/src/worker/held/tests.rs` (held values read as decoded, to the bit); `crates/voxa-app/src/worker/held/spill_tests.rs::a_series_past_the_budget_spills_and_reads_the_same_values` (and after a trip to disk). | serious / rare |
| R-LS-02 | The preview is taken for the full scan, and a small structure missing from it is believed absent. | serious / possible | Labelled while shown; no measuring, picking or plane views on it; replaced in one step (LS-11). | `crates/voxa-app/src/reducer/scene_preview_tests.rs`; `crates/voxa-app/src/ui/scene/captions.rs::a_preview_is_drawn_while_the_volume_builds_and_says_how_far_it_has_got`; a capture of the made-up CT's preview with its caption. | serious / rare |
| R-LS-03 | A slab is drawn out of place or mirrored, or a seam shows, and anatomy appears shifted, doubled or on the wrong side. | serious / rare | One shared slice between slabs; one sampling function; orientation tests with an off-centre bead across a boundary (LS-07). | `crates/voxa-render/src/offscreen_scene/slab_tests.rs::no_seam_where_slabs_meet_and_the_bead_is_where_it_was` (VR, MIP, surface from two sides, and planes across, along and down the slices, through a join and the bead; checked to fail when the shader's slab offset is half a slice out). | serious / rare |
| R-LS-04 | A reduced volume is shown without saying so, and fine detail is judged absent. | serious / possible | Reduced only when nothing else fits, and always said (LS-10). | `crates/voxa-render/src/upload/tests.rs::card_room_halves_only_when_four_slabs_and_its_room_cannot_hold_the_series`; `crates/voxa-app/src/ui/scene/captions.rs::a_reduced_volume_says_by_how_much`. | serious / rare |
| R-LS-05 | Voxa uses so much memory that the computer slows or another program (SolidWorks) loses work. | serious / likely (0.6.0) | Held once at 16 bits (LS-01, LS-03); a budget, past which slabs wait on disk (LS-14); wgpu asked to set aside memory in small blocks. | Measured budgets B5-B7 and B13 (measurement record); `crates/voxa-app/src/budget/tests.rs`; `crates/voxa-render/src/gpu/tests.rs::the_device_reserves_memory_in_small_blocks`. | minor / rare |
| R-LS-06 | Voxa stops when memory runs out while opening a large series, and unsaved measurements are lost. | minor / possible | Memory checked before building (LS-16); the budget; refusing the processor's curve past it (LS-19). | `crates/voxa-app/src/worker/held/memory_tests.rs` (a promised-memory figure made small). | minor / rare |
| R-LS-07 | Pixels from a person's scan are left on disk, where someone else using the computer finds them. | serious / possible | Spill only past the budget, in the local folder, opened by Voxa alone, no names, deleted with its volume, on close and at the next start (LS-15). | `crates/voxa-app/src/spill/tests.rs`; `crates/voxa-app/src/worker/held/spill_tests.rs::a_series_within_the_budget_never_spills` and the file going with its volume. A stop of the computer itself leaves the file until Voxa next starts. | serious / rare |
| R-LS-08 | Voxa stops while a scan loads (the WARP race), losing what was not saved. | minor / likely on WARP | Only the window's thread uses its card (LS-04). | `tools/measure/warp_race.ps1`, 20 runs on Windows' software graphics with resizing (B12). | minor / rare |
| R-LS-09 | A graphics card refuses a volume it was offered, and the window halves it from a 32-bit copy of the whole series on its own thread: the window freezes for seconds, and for a long series the copy can run memory out, losing what was not saved. | minor / rare | None yet: a follow-up, decided by the owner on 31 October 2026. Only a card that refuses a volume it said it had room for meets it. | No card measured refuses (the RTX and the Intel card, on the made-up studies). | minor / rare |
