# Measurements: very large scans

> Volsa was earlier called Voxa. This record keeps the name it was written with.

What opening very large series costs in time and memory, before this work (0.6.0) and after
it, measured against the budgets of the design
(`docs/superpowers/specs/2026-10-30-voxa-large-scans-design.md`, section 4).

Machine: Windows 11, 20 threads, 32 GB of memory, an NVMe disk, an NVIDIA GeForce RTX 5070 Ti
(16 GB, Vulkan) and Intel UHD Graphics 770. Build: `--release`, Rust 1.99.0. Other Claude
sessions were building and testing on the same machine throughout, so times are noisy, and
files were pushed out of Windows' file cache between runs more than they would be on a quiet
computer; memory is steadier.

Every study here is made up by `write_phantom` and is nobody's scan:

- head: 0.6.0's own made-up 512 × 512 × 300 study (150 MiB at 16 bits);
- CT: 512 × 512 × 2,000 slices of a made-up body in Hounsfield units (1,000 MiB at 16 bits),
  as a folder, a compressed zip (420 MB) and a stored zip (1.05 GB);
- body: a whole-body MR of five overlapping stations and the stations joined, 512 × 384 × 832
  (312 MiB at 16 bits), as a folder and a compressed zip.

The tools are in `tools/measure/` (see its README). `measure_big` opens the largest series
without a window, prints each step with the time since it started, hands the volume to the
card as the window does (32 MiB a step) and draws timed pictures; `sample.ps1` samples the
program's private memory every 25 ms from outside; `repeat.ps1` runs every study on both cards
and the real program in its picture-and-quit mode; `cold.ps1` drops a study from the file cache
first; `warp_race.ps1` resizes the window on Windows' software graphics while a scan loads;
`summarise.ps1` prints medians.

## How memory is counted

**Windows charges what is on the graphics card to the program's own memory.** A 3D texture of
500 MiB made on the RTX added 505 MiB to Voxa's private memory, on Vulkan and on DirectX 12
alike, while its working set did not move: the system promises memory to back the card's, but
does not use it. On the Intel card the texture is in the computer's own memory, and the working
set grows too. So a series in full detail counts twice in "peak private memory": once as Voxa
holds it, once as the card does. 0.6.0 had the same charge for its (halved) copy.

The budgets were written for what Voxa itself holds, and on 31 October 2026 the owner decided
that they leave the card's copy out. They are judged on:

- **own**: peak private memory less the card's copy (the size of the values on the card, which
  `measure_big` prints), with the full peak beside it;
- **open** (headless only): own memory up to the moment the volume is on the card, before
  `measure_big` draws its 4K test pictures, which add about 190 MiB of targets and copies that
  the window, drawing into itself, never makes.

**wgpu set memory aside in large blocks.** By default it reserved 128 MiB of the card's memory
and 64 MiB of the computer's for the first small thing made, and a block of 256 MiB for a
150 MiB volume, all counted as Voxa's. Both devices now ask for small blocks on a card
(`MemoryHints::MemoryUsage`); the headless base fell from about 290 MiB to about 100 MiB.

## Before and after (RTX, medians of three)

| Study | | First slice | 3D preview | 3D in full detail | Peak private | Own | On the card |
|---|---|---|---|---|---|---|---|
| head | 0.6.0 | 0.04 s | | 0.35 s | 1,322 MiB | about 1,170 | full |
| | after | 0.04 s | 0.05 s | 0.21 s | 780 MiB | 630 (open 382) | full |
| CT, folder | 0.6.0 | 0.79 s* | | 2.88 s | 4,707 MiB | about 4,580 | halved: 256²×1000 |
| | after | 0.19 s | 0.27 s | 1.44 s | 2,275 MiB | 1,275 (open 1,191) | **full**: 512²×2000 in two textures |
| CT, compressed zip | 0.6.0 | 0.75 s* | | 3.67 s | 4,705 MiB | | halved |
| | after, warm | 0.10 s | | 1.33 s | | | full |
| | after, cold | 0.38 s | 0.50 s | 1.65 s | 2,386-2,445 MiB | about 1,400 | full |
| CT, stored zip | 0.6.0 | 0.94 s* | | 3.80 s | 4,708 MiB | | halved |
| | after, warm | 0.07 s | | 1.16 s | | | full |
| | after, cold | 0.24-0.28 s | 0.35-0.40 s | 1.48-1.57 s | 2,408-2,411 MiB | about 1,400 | full |
| body, folder | 0.6.0 | 1.09 s* | | 1.54 s | 2,225 MiB | about 1,910 | full |
| | after | 0.16 s | 0.19 s | 0.49 s | 1,112 MiB | 800 (open 723) | full |
| body, compressed zip | 0.6.0 | 0.63 s* | | 1.23 s | 2,215 MiB | | full |
| | after, warm | 0.10 s | 0.13 s | 0.51 s | 1,108 MiB | 796 | full |

\* Partly cold in 0.6.0: each run's 4.7 GB pushed the export out of the file cache. "Own" for
0.6.0 is its peak less its card copy (16-bit floats of what it held on the card). The CT
folder opened cold takes 0.15 s to its first slice and 1.5 s to full detail.

On the Intel card, after: head 0.26-0.36 s to full detail, 635 MiB peak (own 485); CT 2.2 s,
2,382 MiB (own 1,382), in full detail in one texture of 2,000 slices (the card's longest 3D side
is 2,048; 0.6.0 halved it); body 0.4-1.2 s, 1,047 MiB (own 735).

The real program, picture-and-quit on the RTX (medians of three): head 668 MiB (0.6.0: 1,463),
CT 2,621 MiB, own 1,621 (0.6.0: 4,859), body zip 1,228 MiB, own 916 (0.6.0: 2,491).

Pictures drawn and read back, RTX: VR 1280 × 800 head 1.8 ms (0.6.0: 4.7), CT 3.8 ms in full
detail (0.6.0: 20.9, halved), body 1.7 ms (0.6.0: 4.3); at 3840 × 2160, 12.0, 16.3 and 12.5 ms
(0.6.0: 35.5, 41.5, 32.4), most of the gain from reading the picture back a row at a time.

## Against the budgets

| Budget | head | CT | body | Met |
|---|---|---|---|---|
| B1 first slice | 0.3 s: 0.04 | 0.5 s: 0.19 (zips cold 0.24-0.38) | 0.5 s: 0.16 (zip 0.10-0.37) | yes |
| B2 3D preview | (none needed): 0.05 | 1.0 s: 0.27 (zips cold 0.35-0.50) | 0.8 s: 0.19 | yes |
| B3 full detail | 0.6 s: 0.21 | 2.0 s: 1.44 (zips cold 1.5-1.7) | 1.2 s: 0.49 | yes |
| B4 detail on both cards | full | full: 512²×2000 | full | yes |
| B5 headless, own (open) | 550: 630 (382) | 1,500: 1,275 (1,191) | 750: 800 (723) | up to the volume on the card, yes; with the tool's 4K pictures, head and body 80 and 50 MiB over |
| B6 `voxa.exe`, own | 750: 518 | 1,700: 1,621 | 950: 916 | yes |
| B7 held after building | 221: 193 (phase 1) | 1,114: 941 | 392: 318 | yes |
| B8 VR 1280 × 800, RTX | 8: 1.8 ms | 33: 3.8 ms | 8: 1.7 ms | yes |
| B9 a 4K picture read back | 15: 12.0 ms drawn and read back | | | yes |
| B10 zip against folder, to full detail | stored ≤ 1.2 ×, compressed ≤ 1.5 × | warm 0.96 × and 1.10 ×; cold 1.0 × and 1.1 × | warm 0.9 × | yes, after the last change below |
| B11 a window frame while uploading, RTX | 16 ms: longest step 11 ms | longest 25 ms (most steps about 11) | longest 32 ms | no for the longest step of a long series |
| B12 software graphics, resized while loading | none of 20 runs stopped (0.6.0: 3 of 3) | | | yes |
| B13 budget 512 MiB, CT | in full detail in 1.5-1.8 s, 488 MiB of its values in memory and none past the budget; own memory 905-927 MiB in three runs (base + budget + 200 is about 812), working set 673-692 MiB | | | the volume's count and the working set yes; private memory about 100 MiB over |

## What changed on the way, from measuring

- **Phase 1** (commit `8eddcfc`): the head peaked at 928 MiB headless and the CT at 2,048 MiB,
  still halved. Most of the rest was the card's copy and wgpu's large blocks (above).
- **The window's step** was 64 MiB, which took about 21 ms a frame on the RTX; it is one 32 MiB
  slab, about 11 ms. The longest step of a long series is still 25-32 ms: the first, which makes
  the textures, and the last, which works out the blocks of largest values.
- **Zips** looked slow in phase 1 only because they were read cold. Opened cold after the other
  changes, the compressed CT zip took 8.8-9.3 s to decode into its volume and the stored zip
  2.2 s, against 1.0 s for the folder: every reader shared the zip's one file handle, which
  Windows reads through in turn, 8 KiB at a time. Each reader now opens the zip itself and reads
  64 KiB ahead: 1.2 s and 1.1 s cold.
- **Past the budget**, the preview first put every eighth slice into the volume, which made
  every slab of the CT at once (1.4 GB with a 512 MiB budget); the preview's slices are now
  decoded on their own and the volume filled slab by slab.
- **Indexing** a folder of 2,000 slices took 357 ms in the bench before and 183 ms after.
- **From the final review**: counting what a volume holds no longer takes its slabs' locks, a
  slab goes to disk still readable, a slice or slab on disk is read from the file without being
  brought back, and a hand-over interrupted by another series lets the old volume go. With a
  512 MiB budget the CT then reached full detail in 1.5-1.8 s, where it had taken 2.3-7.7 s
  while slabs were brought back and put away again during the hand-over.

## What is left

- B11 for the first and last step of a long series' hand-over, which make the textures and the
  blocks of largest values.
- B13's private memory, about 100 MiB past base + budget + 200 MiB while a series past the
  budget goes to the card: the slab read from the file for each step, its texels and the
  card's staging, each 32 MiB, on top of the budget. The volume's own count is within it.
- The headless head and body runs pass B5 only up to the moment the volume is on the card.
- Smooth planes are not offered for a series the card holds in more than one texture (the
  2,000-slice CT on the RTX, which holds 512 MiB a texture).
- A computer that stops leaves its spill file until Voxa next starts.
