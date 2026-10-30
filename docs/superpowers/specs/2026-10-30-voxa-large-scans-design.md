# Very large scans, fast and within a memory budget: design

Date: 2026-10-30
Status: draft for the owner's approval. Choices marked *my call* are mine, for the owner to
overrule. Branch `claude/big-scans-c8aec7`, cut from `release` (0.6.0, ddc5686).

## 1. Why

Voxa is growing to CT and whole-body MRI: series of 500 to 3,000 slices, three to ten times
a head MRI. The owner said on 2026-10-29: "for the really large files, this still needs the
best performance possible", and has asked more than once for memory that stays bounded: the
computer often runs SolidWorks, and a leak elsewhere once used up all the memory Windows
could promise.

Voxa 0.6.0 was built for head MRI. Measured on made-up studies (section 3), it holds a
2,000-slice CT as 32-bit numbers, briefly twice over: 4.7 GB at the peak and 2.7 GB after,
for 1 GB of 16-bit values. It shows that CT in 3D at half its resolution, plane views
included, because the volume is halved until it is under 512 MiB and under the card's
largest 3D side. And with only Windows' software graphics (WARP), resizing the window
while a scan loads stops Voxa (section 3.4).

This work makes very large series open quickly and look as fine as the card allows, with
the memory Voxa keeps for scans held under a set budget.

## 2. Goals, non-goals, anti-goals

Goals:

- G1. Each series held once, in its own 16-bit form where that is exact: no second copy
  in ordinary memory, and none kept beside the card's copy when memory is short.
- G2. The first slice on screen at once; a 3D preview within about a second; full detail
  after, without the window stalling.
- G3. Full detail for series longer than the card's largest 3D texture, by splitting the
  volume into slabs along its slices.
- G4. The memory Voxa keeps for decoded scans held under a budget; past it, decoded slabs
  go to a file of Voxa's own on disk and come back when needed.
- G5. No crash from the window and a worker using the card at once (the WARP crash).
- G6. Budgets written down (section 4) and checked by measurement on made-up large studies,
  before and after.
- G7. The standards' documents kept: requirements traced to tests (LS-nn), risk entries
  (R-LS-nn), security and usability notes.

Non-goals:

- New views, CT windows or anything CT-specific on screen (the redesign's View tab does
  that); joining whole-body stations into one volume (a later whole-body project); any
  change to how labelling finds structures.
- Opening compressions Voxa cannot read today (JPEG-LS and others).
- Speeding up drawing itself, beyond what holding values as stored gives.

Anti-goals (it fails even if it works):

- A preview, or a volume shown at reduced detail, taken for the scan in full. It is always
  said, and nothing is measured on it.
- A value read differently from 0.6.0: a measurement or a value under the pointer must not
  change by one unit.
- Patient pixels left on disk after Voxa closes, or written there for a series that fits.
- A window that stutters while a large series loads.

## 3. What was measured (0.6.0, 2026-10-30)

### 3.1 How

Two tools, both committed with this work: `write_phantom` gains `ct` (a 512 × 512 CT of a
made-up body from the thighs to the head, 2,000 slices 0.625 mm apart, Hounsfield units
stored with intercept −1024; 1.0 GB), `body` (a whole-body MR in five overlapping stations
of 512 × 384 × 192, each a series, and the stations joined into one series of 832 slices)
and `zip` (any folder, compressed or stored). `measure_big` opens an export as the window
does, with no window, gives the window's graphics device to the workers as the window
does, and prints when the first slice is decoded, when the volume is built, what the card
holds, and how long pictures take to draw and read back. The program's private memory was
sampled every 25 ms from outside, and the real `voxa.exe` was also run in its
picture-and-quit mode (`VOXA_CAPTURE`).

Machine: 20 threads, 32 GB, NVMe disk, RTX 5070 Ti (16 GB, Vulkan) and Intel UHD 770.
Other Claude sessions were running; about 8 GB of memory was free. Medians of three runs.
The head-like series is 0.6.0's own made-up 512 × 512 × 300 study, for comparison with the
0.6.0 report.

### 3.2 Opening (headless, RTX)

| Study | Values at 16 bits | First slice | 3D built | Peak private | On the card |
|---|---|---|---|---|---|
| Head-like 512²×300 | 150 MiB | 0.04 s | 0.35 s | 1,322 MiB | full |
| CT 512²×2000, folder | 1,000 MiB | 0.79 s* | 2.88 s | 4,707 MiB | half: 256²×1000 |
| CT, compressed zip (420 MB) | | 0.75 s* | 3.67 s | 4,705 MiB | half |
| CT, stored zip (1.05 GB) | | 0.94 s* | 3.80 s | 4,708 MiB | half |
| Whole body joined 512×384×832, folder | 312 MiB | 1.09 s* | 1.54 s | 2,225 MiB | full |
| Whole body, compressed zip | | 0.63 s* | 1.23 s | 2,215 MiB | full |

\* Partly cold: each run's 4.7 GB pushed the export out of Windows' file cache. With the
files cached the CT's first slice took 0.1 s; most of the wait is reading 2,000 headers.

On the Intel card: head 0.62 s to 3D, 1,461 MiB; CT 2.02 s, 4,680 MiB, also halved (the
card's largest 3D side is 2,048); whole body 1.09 s, 2,455 MiB.

The real program, picture-and-quit on the RTX: head 1,463 MiB peak (0.6.0's report: 1,590);
CT 4,859 MiB; whole-body zip 2,491 MiB. After the CT opened, 2,660 MiB stayed held.

Where the CT's memory goes: every slice decoded to 32-bit floats and kept until the end
(2 GiB), stacked into one more 32-bit array (2 GiB), halved (256 MiB) and turned into 16-bit
floats for the card (128 MiB). The stacked array stays for the 2D slices, measuring and
picking. The frame cache may hold another 1 GiB of 32-bit slices. The CPU time is 17-19 s
for 2,000 slices, about 8.5 ms a slice, though decoding one takes about 1.4 ms: three preset
windows are worked out for every slice decoded for a volume, shown or not.

### 3.3 Drawing (headless, a picture drawn and read back)

| Study | VR 1280×800 RTX | VR 3840×2160 RTX | VR 1280×800 Intel |
|---|---|---|---|
| Head-like | 4.7 ms | 35.5 ms | 29.7 ms |
| CT (halved) | 20.9 ms | 41.5 ms | 44.6 ms |
| Whole body | 4.3 ms | 32.4 ms | 14.6 ms |

At 4K most of the time is reading the picture back: `Gpu::read_back` copies byte by byte
(44 ms at 4K against 12 ms row by row in the 0.6.0 measurements).

### 3.4 The crash with software graphics

Reproduced with a throwaway build that asks for WARP: with a scan opened at launch and the
window resized while it loads, three runs of three stopped with
`Surface::configure: GpuWaitTimeout` on the main thread. wgpu waits for the card to finish
before it reconfigures the window, and fails if another thread hands the card work during
that wait; Voxa's worker does exactly that while it uploads a volume. With uploads left to
the window's thread, four runs of four were clean. On a real card the wait is short, so the
race is rare, but it is the same race.

### 3.5 Zips

0.6.0's zip work holds: a zip costs 1.27-1.32 times a folder for the CT, most of it
inflating or checking 1 GB, and a cold first open was no slower than a warm one on this
disk. Zips need no new design; they are kept within a budget (B10).

### 3.6 The card's limits

The window asks wgpu for its default limits, whose largest 3D texture is 2,048 a side. The
RTX on Vulkan offers 16,384, the Intel card 2,048; DirectX 12 never offers more than 2,048.
So a longer series needs slabs whatever is asked for, and a series that fits needs the card's
own limits to be asked for.

## 4. Budgets

On the machine of section 3, with the export in Windows' file cache, made-up studies,
medians of three. The headless tool's own base (its device and the empty session) is about
300 MiB; the window's is about 445 MiB (0.6.0 report).

| No. | What | Head-like 512²×300 | CT 512²×2000 | Whole body 512×384×832 |
|---|---|---|---|---|
| B1 | First slice decoded, from opening | ≤ 0.3 s | ≤ 0.5 s | ≤ 0.5 s |
| B2 | 3D preview ready | (no preview needed) | ≤ 1.0 s | ≤ 0.8 s |
| B3 | 3D in full detail ready | ≤ 0.6 s | ≤ 2.0 s | ≤ 1.2 s |
| B4 | Detail on the RTX and the Intel card | full | full: 512²×2000 | full |
| B5 | Peak private memory, headless | ≤ 550 MiB (1,322) | ≤ 1,500 MiB (4,707) | ≤ 750 MiB (2,225) |
| B6 | Peak private memory, `voxa.exe` | ≤ 750 MiB (1,463) | ≤ 1,700 MiB (4,859) | ≤ 950 MiB (2,491) |
| B7 | Held after opening, above the base | ≤ 1.05 × values at 16 bits + 64 MiB | | |
| B8 | Still VR picture 1280×800, RTX, drawn and read back | ≤ 8 ms | ≤ 33 ms | ≤ 8 ms |
| B9 | Reading back a 3840×2160 picture | ≤ 15 ms | | |
| B10 | Zip against folder, time to full detail | ≤ 1.3 × | ≤ 1.3 × | ≤ 1.3 × |
| B11 | Window frame while a volume uploads | ≤ 16 ms on the RTX | | |
| B12 | WARP, scan at launch, window resized while loading | no stop in 20 runs | | |
| B13 | With the budget set to 512 MiB, held in ordinary memory | ≤ budget + 64 MiB, and the CT still opens in full detail | | |

Moving pictures keep 0.5.0's rule (drawn at full, ¾ or ½ size to fit 8 ms), which this work
does not change. On a slower computer the times grow; the memory budgets do not.

## 5. Approaches

**A. One volume held once, at 16 bits, in slabs, with a budget (recommended).** Keep the
present structure (workers decode, a store holds the one volume, the card draws from a 3D
texture) and change what is held and how it moves: values in their stored 16 bits, decoded
straight into slabs of whole slices, no 32-bit copy kept; the card's copy in up to four slab
textures, uploaded from the window's thread a little each frame; a small preview first;
slabs past the budget written to a file and read back on demand. Each step can be built and
measured on its own. Risk is moderate: the volume's type, the shaders' one sampling
function, and the worker's build change.

**B. Bricks and a pyramid streamed on demand.** Cut the volume into small bricks (64³) at
several resolutions; the card keeps a cache of bricks in one atlas texture with a table
saying where each is; the shader picks the resolution each ray needs and asks for missing
bricks, which a worker streams from a disk cache. It handles any size on any card with
fixed memory on both sides, as large-data viewers do. It is a rewrite of the volume, plane
and Parts shaders, of empty-space skipping and of uploading; filtering across bricks needs
a border on each; and it is weeks of work with more ways to draw something wrong. For
1 to 6 GB series on cards of 4 to 16 GB it buys little over A.

**C. Keep 32-bit values; remove the duplicates and raise the limits.** Decode into place,
drop the frame copies, ask for the card's limits, split long series into slabs. Least
change, but a 2,000-slice CT still holds 2 GiB and a 3,000-slice one 3 GiB, with no bound,
and the card gets a second conversion pass.

A meets the budgets with the least risk, and leaves B possible later for series larger
than any card (section 9). *My call*: A.

## 6. The design (approach A)

### 6.1 Holding values as stored

A new kind of volume in `voxa-core`, the *held volume*, keeps a series' values as the files
store them: 16-bit whole numbers, unsigned or signed, with the series' one rescale (slope and
intercept). Any value read from it is the value 0.6.0's decoding gives, to the bit: the
32-bit number `dicom-pixeldata` makes from the stored value with that slope and intercept
(LS-02). A series that cannot be held exactly so is held as 32-bit floats, as today: stored
values of more than 16 bits, floating-point pixel data, slices whose rescales differ,
`MONOCHROME1` (whose inversion is per slice), and colour.

The held volume is split into slabs: runs of whole slices of at most 32 MiB *(my call)*.
Each slab is in memory or in the spill file (6.6). Readers ask for a value, a line of values,
a slice as 32-bit numbers, or a slab's raw 16-bit values; none needs the whole volume at
once.

`Volume` (32-bit, whole) stays as it is for what is small or transient: labelling's input,
the label tools, tests. A trait over both (dimensions, placement, the value at a voxel, a
slice into a buffer) lets `voxa-volume`'s sampling, lines of sight, regions and planes take
either, so `voxa-segment` is not touched (the plugin session works there).

### 6.2 Building a volume

The worker that builds a series' volume:

1. Checks the memory first (LS-16): the values at 16 bits must fit in what Windows can
   still promise Voxa, less a margin of 512 MiB *(my call)*; if not, the 3D view says so
   in a sentence and nothing is built.
2. Decodes the slice on screen first (it is already in hand for the 2D viewport), then, for
   a series of more than 256 slices *(my call)*, every eighth slice for the preview (6.4),
   then the rest slab by slab, each slab's slices in parallel, each decoded straight into
   its place in the slab: no list of decoded slices, no stacking.
3. Works out a slice's three preset windows only when that slice is shown (LS-06). The
   windows are the same; only when they are worked out changes.
4. Puts each slab in the store as it is finished; the 2D viewport reads finished slabs at
   once and decodes on its own only a slice whose slab is not finished.
5. Fails the whole build if a slice cannot be decoded, with today's sentence; a slab is
   never shown with slices missing.

The frame cache holds 16-bit slices too, and shares the one budget (6.6).

### 6.3 The card

- The window's device asks for the card's own limits for the largest 3D texture and the
  largest buffer, and for 16-bit normalised textures where the card offers them (LS-08).
- The card holds the values as stored: `R16Unorm` for unsigned, `R16Snorm` for signed, with
  the rescale and window applied in the shader; a card without them gets 16-bit floats,
  converted slab by slab as they go (LS-09). The 0.6.0 conversion pass and its copy go.
- A series longer than the card's largest 3D side, or larger than one texture may be
  (512 MiB *(my call)*, lowered if the card refuses), is held in up to four slab
  textures along the slices. Neighbouring slabs share one slice, so a ray reading between
  two slices never needs both textures. The volume, plane and Parts shaders read the value
  through one function that picks the slab; the blocks of largest values and the smooth
  planes' curve are split the same way (LS-07).
- A series that does not fit in four slabs, or in the room the card has (Windows' own count
  of what Voxa may use on that card, as labelling reads it today), is reduced, halving
  along the slices first when its slices are closer than its pixels, and the 3D view says
  so as today (LS-10). On the RTX and the Intel card the study CT fits in full.
- Only the window's thread hands work to the window's card (LS-04). A worker prepares
  slabs; each frame the window hands the card finished slabs up to 64 MiB *(my call)*,
  and asks for another frame while any are waiting. This removes the WARP race by
  construction. The worker's own hand-over of 0.6.0 goes.
- `read_back` copies whole rows (LS-05).

### 6.4 Preview, then full detail

For a series of more than 256 slices, once every eighth slice is decoded the worker makes
a preview: those slices, each reduced to a quarter across and down by averaging, placed at
their own positions. For the CT that is 128 × 128 × 250, 8 MB, on the card at once. The 3D
view draws the preview and says "Preview: loading full detail" with a count (the redesign
shows it in its strip; until then, in today's line above the 3D picture). When every slab is
on the card the full volume replaces the preview in one frame and the words go.

While the preview is shown, nothing in 3D is measured or picked: the pointer names no part
and a click sets no position, and a plane view of the preview is not offered (LS-11). The
2D slices are always the full slices, decoded as in 6.2.

### 6.5 First slice and indexing

The slice on screen is decoded before anything for 3D (it is today; this work keeps it so
while the build is reworked). Indexing reads headers on several threads, each through a
reader of its own, as decoding already does (LS-13), which matters most when the export is
not in the file cache.

### 6.6 The memory budget and the spill file

One budget covers the decoded values Voxa keeps in ordinary memory: the held volume's slabs
and the frame cache together. *My call*: 2 GiB, or a quarter of the computer's memory if
that is less; kept in `preferences.json`, with no control for it in the window until the
redesign gives it a place. The frame cache keeps at least 64 MiB of it.

- A series whose values fit in the budget stays in memory whole after it is built, as
  today: no disk is touched.
- Past the budget, a finished slab that is on the card is written to the spill file and
  let go. A slab asked for later (a slice to show, a region to measure, a line to pick
  along, labelling) is read back and kept while there is room, the slab least lately used
  going first.
- On an integrated card the card's copy is in the same memory, so the slabs held count
  against the budget together with what the card holds *(my call)*.
- The spill file is in Voxa's local folder (never the roaming one), opened by Voxa alone,
  named by a random number, holding only 16-bit values and no header, name or tag. It is
  deleted when its volume gives way, when Voxa closes, and, if Voxa was stopped, at the next
  start (LS-15).

### 6.7 Labelling and other whole-volume work

Labelling is given a 32-bit `Volume` made from the held volume when a run starts, and let go
when the run ends; the memory check before a run counts it, as it counts the networks
today. A model that takes part of a series (a section along its slices) is given just that
section, through the same call with a range: the hook the plugin engine needs (LS-18). The
smooth planes' curve worked out on the processor, for cards that cannot work it out
themselves, is refused for a series past the budget, with a sentence (LS-19).

### 6.8 Order of building

1. Phase 1, held once: read-back rows; preset windows only when shown; the held volume at
   16 bits; decoding straight into slabs; the card holding values as stored; uploads only
   from the window's thread. Meets B5-B9, B11, B12 for series that fit one texture.
2. Phase 2, full detail and speed: the card's own limits; slab textures; the preview;
   indexing in parallel. Meets B1-B4.
3. Phase 3, the budget: the spill file, the memory check before building, labelling's
   input by section, the CPU curve refused past the budget. Meets B13.

Each phase ends with `cargo xtask check` green and the measurements of section 3 run again.

## 7. Requirements

To be added to `docs/quality/requirements.md` (started by the redesign) as a section of its
own. "Shown by" is filled in by the plan as tests are written.

| No. | Requirement | Design | Risk |
|---|---|---|---|
| LS-01 | A series' decoded values are held at most once in ordinary memory; no 32-bit copy is kept beside a 16-bit one. | 6.1, 6.2 | R-LS-05 |
| LS-02 | Every value read from a series held in 16 bits equals, to the bit, the value 0.6.0's decoding gives, for unsigned, signed, 8-bit and rescaled values; a series that cannot be held so exactly is held in 32 bits. | 6.1 | R-LS-01 |
| LS-03 | Building a volume never holds every decoded slice beside the volume; the peak while building is within B5 and B6. | 6.2 | R-LS-05 |
| LS-04 | No thread but the window's hands work to the window's graphics card; resizing the window while a volume loads never stops Voxa (B12). | 6.3 | R-LS-08 |
| LS-05 | A picture read back from the card is copied a row at a time (B9). | 6.3 | |
| LS-06 | A slice's preset windows are worked out when it is shown, not when it is decoded for a volume, and are the same windows as 0.6.0's. | 6.2 | |
| LS-07 | A series longer than the card's largest 3D side is drawn in full detail from up to four slabs; a picture or plane across a slab boundary equals one drawn from a single texture to within one grey level, and the slabs keep the series' order and orientation. | 6.3 | R-LS-03 |
| LS-08 | The window's device asks for the card's own largest 3D side and buffer, and for 16-bit normalised textures where offered. | 6.3 | |
| LS-09 | The card holds 16-bit values as stored where it can, and otherwise as 16-bit floats converted a slab at a time. | 6.3 | R-LS-01 |
| LS-10 | A series is drawn at reduced detail only when it fits neither four slabs nor the room the card has, and the 3D view then says so. | 6.3 | R-LS-04 |
| LS-11 | For a series of more than 256 slices a preview is drawn first and labelled as one with how far loading has got; while it is drawn nothing is measured or picked in 3D and no plane view is drawn from it; the full volume replaces it when all of it is on the card. | 6.4 | R-LS-02 |
| LS-12 | The slice on screen is decoded before any other slice of its series. | 6.2, 6.5 | |
| LS-13 | Indexing reads files' headers on several threads and gives the same index as 0.6.0. | 6.5 | |
| LS-14 | The decoded values held in ordinary memory stay within the budget (default 2 GiB or a quarter of the computer's memory, whichever is less) plus one slab (B13). | 6.6 | R-LS-06 |
| LS-15 | The spill file is used only past the budget, lives in Voxa's local folder, is opened by Voxa alone, holds no names or tags, and is deleted when its volume gives way, when Voxa closes, and at the next start after a stop. | 6.6 | R-LS-07 |
| LS-16 | Before building a volume Voxa checks that Windows can still promise the memory, and says in a sentence when it cannot instead of failing. | 6.2 | R-LS-06 |
| LS-17 | Opening a zip takes no more than 1.3 times as long as the same files in a folder (B10). | 3.5 | |
| LS-18 | Labelling's input is made from the held volume for the run only, and can be a range of slices. | 6.7 | |
| LS-19 | The smooth planes' curve is not worked out on the processor for a series past the budget, and the person is told why. | 6.7 | R-LS-06 |
| LS-20 | The budgets of section 4 are measured with `measure_big` and `voxa.exe` on the made-up studies, and the results recorded. | 4 | |

## 8. Risk register, security and usability

To be added to `docs/quality/risk-register.md` as a section of its own. Severity and
likelihood use the register's scale.

| No. | Hazard and how harm could follow | Before | Control | Checked by | After |
|---|---|---|---|---|---|
| R-LS-01 | A value held in 16 bits reads differently from the file (sign, slope or intercept mishandled); a measurement or a value under the pointer is wrong. | serious / possible | Exact equality with 0.6.0's decoding for every kind of stored value (LS-02); 32 bits where 16 cannot be exact; the card's copy is for drawing only and measuring always reads the held values. | Tests over every kind the test kit writes, and over the made-up CT. | serious / rare |
| R-LS-02 | The preview is taken for the full scan, and a small structure missing from it is believed absent. | serious / possible | Labelled while shown; no measuring, picking or plane views on it; replaced in one step (LS-11). | Reducer tests; a capture of the preview. | serious / rare |
| R-LS-03 | A slab is drawn out of place or mirrored, or a seam shows, and anatomy appears shifted, doubled or on the wrong side. | serious / rare | One shared slice between slabs; one sampling function; orientation tests with the phantom's off-centre bead across a boundary (LS-07). | Device tests comparing slabbed and single-texture pictures and planes. | serious / rare |
| R-LS-04 | A reduced volume is shown without saying so, and fine detail is judged absent. | serious / possible | Reduced only when nothing else fits, and always said (LS-10). | Test that the words appear whenever the card holds fewer voxels than the series. | serious / rare |
| R-LS-05 | Voxa uses so much memory that the computer slows or another program (SolidWorks) loses work. | serious / likely (0.6.0) | Held once at 16 bits (LS-01, LS-03); a budget (LS-14). | Measured budgets B5-B7, B13. | minor / rare |
| R-LS-06 | Voxa stops when memory runs out while opening a large series, and unsaved measurements are lost. | minor / possible | Memory checked before building (LS-16); the budget; refusing the CPU curve past it (LS-19). | Test with a promised-memory figure made small. | minor / rare |
| R-LS-07 | Pixels from a person's scan are left on disk, where someone else using the computer finds them. | serious / possible | Spill only past the budget, in the local folder, opened by Voxa alone, no names, deleted on close and at the next start (LS-15). | Tests of creation, deletion and leftovers; a check that small series never spill. | serious / rare |
| R-LS-08 | Voxa stops while a scan loads (the WARP race), losing what was not saved. | minor / likely on WARP | Only the window's thread uses its card (LS-04). | 20 runs on WARP with resizing (B12). | minor / rare |

Security notes:

- A file that claims a huge series cannot make Voxa reserve memory it does not have: each
  image keeps 0.6.0's 256 MiB check, and a volume is built only after the memory check
  (LS-16).
- The spill file holds decoded pixels of a person's scan. It is in the local folder, named
  by a random number, opened without sharing, never in the log, and deleted as LS-15 says.
  Disk encryption is the computer's (BitLocker), not Voxa's; this is said in the security
  notes beside `SECURITY.md`'s scope.
- No new third-party component: 16-bit textures, slabs and the spill file use wgpu and the
  standard library. `cargo deny` is unchanged.

Usability notes (for `docs/quality/usability.md`):

- New words on screen: "Preview: loading full detail (n of m slices)"; "Not enough memory
  to show this series in 3D: it needs n GB, and Windows can promise Voxa m GB. Close other
  programs and open it again."; "Smooth planes need more memory than this series leaves.";
  today's "Shown at half resolution to fit this graphics device" stays. Each is checked by
  the redesign's wording test (no finding words).
- A capture of the preview and of the full CT is added to the recorded checks.

## 9. Left for later

- Full detail of a chosen region (the cut box) on a card too small for the whole series:
  approach B's idea applied to one region. Not needed for the studies and cards here.
- Joining whole-body stations into one volume; CT windows and Hounsfield readouts (the
  redesign); JPEG-LS and other compressions; measuring JPEG 2000 decoding on large series
  (the made-up studies are uncompressed).

## 10. Hooks for the other sessions

For the redesign (it does not touch reading, caching, volume building or drawing):

- The scene's state gains how detailed the 3D picture is: preview with a count, full, or
  reduced with the reason. Show it in the strip above the pictures; until then today's line
  shows it.
- The reducer gains "the 3D picture or the planes are on screen". While neither is, a
  series is decoded only around the slice shown and no volume is built. Set it from the
  chosen arrangement (Slices, Slices + 3D, Planes, 3D); until then it is always on, as today.
- Small changes in `ui/`: `start.rs` (the device asks for the card's limits and 16-bit
  textures), `mod.rs` (the workers are no longer given the device), `pictures.rs` and
  `scene/gpu.rs` (the window hands finished slabs to the card each frame). Listed here so
  the redesign can expect them.
- The memory budget is a preference with no control yet; a place for it (an advanced
  setting) is the redesign's to choose.
- `voxa-dicom`: indexing in parallel changes the loop in `index.rs` that visits files, next
  to the spot where the redesign lists documents instead of skipping them. Whoever lands
  second takes the other's change.

For the plugin engine: labelling and other models get their input through one call that
makes a 32-bit `Volume` from the held volume, whole or for a range of slices (6.7).
`voxa-segment` is not changed by this work.

## 11. Open for the owner

1. Approach A, built in the three phases of 6.8.
2. The memory budget's default: 2 GiB, or a quarter of the computer's memory if less.
3. Writing decoded pixels of a series past the budget to a file in Voxa's local folder,
   deleted when done (6.6). The alternative is never to write them: past the budget a
   series would be shown at reduced detail, or not in 3D.
4. Region refinement (section 9) left for later.

## 12. Left to the builder

Names of types and modules; the slab size, the per-frame upload amount, the preview's
stride and reduction, the spill file's name and layout, the exact margin of the memory
check, and how the shaders choose a slab, so long as each requirement holds and the
budgets of section 4 are met.
