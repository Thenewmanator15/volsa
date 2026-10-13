# Any MRI export: design

Date: 2026-10-13
Status: the owner asked for this ("use the review to improve the app", "more for different
types of MRI scans") and chose its scope from a list: more kinds of MRI, and the everyday
fixes, as one piece of work. The choices below marked *default* are mine, for the owner to
overrule.

## What this is for

Voxa was built against one kind of export: a head, a few two-dimensional stacks and a thin
three-dimensional slab. An export from another scanner, another body part or another kind of
examination holds series Voxa either flags as faulty or cannot show: locating images,
diffusion and multi-echo series that hold several volumes, colour maps and report pages. A
review of the application also found everyday gaps: no log, nothing remembered between runs,
a console window on Windows, cut-off names.

This work makes any ordinary MRI export open cleanly, says what each series is, and closes
those gaps. It adds no new view and no new measurement.

## Requirements

### A. Series are described, not accused

1. A series is one of these kinds, worked out from what the index already knows:
   a volume; locating images in several planes; a single image; colour pictures; a plain
   stack with a reason; images Voxa cannot read.
2. The browser's card says the kind in ordinary words for the kinds that are normal in an
   export (locating images, a single image, colour pictures). Only a plain stack with a
   reason, and images that cannot be read, keep the warning mark.
3. Resting the pointer on a card shows the series' full description and everything in B.

### B. What kind of scan

4. From the standard MR tags of a series' first image Voxa reads: the plane (axial, coronal,
   sagittal, or oblique nearest one of them), whether it was acquired in two or three
   dimensions, the family of sequence (spin echo, gradient echo, inversion recovery,
   echo-planar, in combination), repetition, echo and inversion time, flip angle, field
   strength, and the body part named. Classic and many-frame ("enhanced") files both.
5. The card gains one short line of it (plane, dimensions, family); the full list is in the
   hover text. Facts only: Voxa does not guess a weighting such as "T1" or "T2". *default*
6. None of it is taken from, or written with, anything that identifies a person.

### C. Several volumes in one series

7. When every position in a series is filled the same number of times, at least twice, the
   series holds that many volumes (diffusion weightings, echoes, time points). Voxa lists
   each as a volume of the series, each a regular stack with its own geometry check.
8. Volumes are told apart, and named, by the first of these that differs between them and
   is constant within each: temporal position, diffusion b-value, echo time, trigger time,
   acquisition number. With none, they are numbered in the order of their images.
9. The browser shows one card for the whole series, saying how many volumes it holds. The
   viewport says which volume is shown and steps between them with the `[` and `]` keys and
   two buttons; the slice, the zoom and the 3D camera stay where they are. A window chosen
   by hand stays; an automatic window is chosen again for the new volume. *default*
10. A series whose repeats are not such a grid stays a plain stack and says why, as now.
11. The series opened first by itself is never a later volume when an earlier one ties.

### D. Colour pictures

12. Images stored in colour (RGB, and the YBR forms the DICOM library converts) are shown in
    the 2D viewport and as thumbnails, in their own colours, with zoom, pan, flips and turns.
    They have no window, no 3D view, no planes and no region statistics, and each of those
    says so in a sentence. Palette colour is refused in a sentence. *default*

### E. Reading more files, faster and more surely

13. Files stored as "deflated" explicit little endian are read.
14. JPEG and JPEG 2000 decoding each have an automated test on a made-up picture.
15. A file holding many frames is parsed once for a run of requests, not once per frame.
16. The automatic window does not collapse when an image holds a few values far outside
    the rest.

### F. Everyday use

17. A log file in the folder the system gives for an application's data: when Voxa started
    and with which version, system and graphics device; how many studies and series an
    export held; every failure Voxa tells the person about, in Voxa's own words; a crash,
    with where it happened. Never a patient's name, identifier or date, never a value from a
    file, never a path. The About window says where the file is. It is cut back when it
    passes 1 MB.
18. Between runs Voxa remembers the window's size, the layout, and the switches for the
    radiological convention, linking and how labels are drawn. A file that is damaged or
    cannot be written costs nothing but the memory. A run that takes a picture of the window
    reads and writes none of it.
19. Voxa does not remember which scans were opened: that would keep paths, which can name a
    person, and the settings folder is promised to hold nothing that says whose scan it
    was. *default*, and a departure from the scope as listed to the owner, who is told.
20. On Windows a release build opens no console window. What it would have printed (the
    help, the version, a mistake on the command line, a failure to start) is shown in a
    small message window instead.
21. The window has an icon, drawn by the program.
22. The interface library's support for screen readers is switched on.
23. Tests that wait a fixed time for something another thread does are made to wait for the
    thing itself.

## What it does not do

No recent-scans list (19). No weighting guess (5). No palette colour, JPEG-LS or JPEG XL.
No film-like playback through the volumes of a series. No tilted planes, no moving of
annotations, no installer, no third-party licence bundle: all are in the review and are for
later work. The version number does not change; this lands under "Unreleased".

## Rules carried over

The disclaimer verbatim. No network, no telemetry. Scan data never in the repository, and
nothing from a real export in a test, a document or a commit message: no name, date,
description, identifier, size, count, spacing or position; made-up values, checked against
the owner's export before each commit. No `unsafe`. Library crates never panic. Every
dependency on the allow-list. `cargo xtask check` before every commit.
