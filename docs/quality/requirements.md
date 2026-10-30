# Requirements

Each requirement has a number that never changes or is reused, says what Voxa must do in
words a tester can check, and names what shows it is met: an automated test (by its path
and name) or a recorded check (by its entry in the [usability file](usability.md)). A
requirement with nothing in "Shown by" is not yet met.

Kept in the spirit of IEC 62304: software requirements traced to their verification.
Certification is not a goal; this keeps the door open.

Earlier work is described by its design documents under `docs/superpowers/specs/` and its
acceptance records `docs/acceptance-*.md`; its requirements are brought into this list when
that work is next changed.

## UI: the window (design: 2026-10-30-voxa-ui-redesign-design.md)

| No. | Requirement | Design | Risk | Shown by |
|---|---|---|---|---|
| UI-01 | The window has one top bar holding Open, the study's name and date, the search box, help, theme and a menu; a rail of scans at the left; the pictures in the middle; a panel with the tabs View, Measure, 3D, Labels and Tools at the right; and a bottom line holding the disclaimer and the notices. | 7.1 | | |
| UI-02 | The disclaimer "Voxa is not a medical device and is not for diagnostic use." is on screen, word for word, in every window size from 640×480 up. | 7.1, 7.14 | | |
| UI-03 | Every action a person can take is described once, in one list, with its name, other search words, key, place, when it can be used and why not, and what it does. | 7.2 | | |
| UI-04 | Every button, menu item and key reads its name, hint and key from that list; a control not in the list is a test failure. | 7.2 | | |
| UI-05 | Search, opened by Ctrl+K, `/` or a click, finds every action by its name and by each of its other words, and finds parts, scans and help topics; Enter runs the chosen action. | 7.3 | R-UI-07 | |
| UI-06 | An action that cannot be used now is listed with the reason, and running it posts the reason instead of doing nothing. | 7.3 | | |
| UI-07 | An action that throws work away asks first or can be undone, from any place it is run. | 7.3 | R-UI-07 | |
| UI-08 | Nothing typed into search is kept between runs. | 7.3, 7.16 | | |
| UI-09 | Scans (every series of pictures of the body, including single images, many-frame series and series Voxa cannot read yet) are listed before all other files; locating images, reports and other documents, and series whose Modality is SC, OT or DOC are in a group "Other files", closed by default. | 7.4 | | |
| UI-10 | Each scan's card shows a plain name and, beneath it, the scanner's own description unchanged. | 7.4 | R-UI-01 | |
| UI-11 | A plain name is made only from what the files say; no weighting is worked out from timings. | 7.4, 9.1 | R-UI-01 | |
| UI-12 | A series shown as a plain stack, or that cannot be read, carries a mark on its card; its full sentence is in the card's hover text and is posted as a notice when it is opened. | 7.4, 7.11 | R-UI-06 | |
| UI-13 | While patient details are off, the patient's name and identifier appear nowhere in the window, and a scanned page's card shows an icon instead of a thumbnail. Whatever the setting, Voxa never writes them into a thumbnail, a saved picture, a log line or a saved file's name. | 7.4, 7.12 | R-UI-05 | |
| UI-14 | When an export holds studies of more than one person, the rail says so and groups the studies by person. | 7.4 | R-UI-05 | |
| UI-15 | The pictures are shown in one of four arrangements chosen from a strip: Slices (1, 2 or 4), Slices + 3D, Planes, 3D. | 7.5 | | |
| UI-16 | Voxa opens an export on Slices with the remembered number of viewports (one on a first start), showing the best scan as chosen today; with no such scan, the scan with the most pixels; with no scans, the first of the other files. | 7.5 | | |
| UI-17 | Each scan has a profile, CT, MR or basic, chosen from its Modality. | 7.6 | R-UI-02 | |
| UI-18 | For a CT scan the View tab offers the windows brain (W80 L40), soft tissue (W400 L40), lung (W1500 L−600) and bone (W1800 L400), and Auto. | 7.6 | | |
| UI-19 | The value under the pointer is shown; for a CT scan whose rescale says HU or says nothing, in HU; otherwise with no unit unless the file names one. | 7.6 | R-UI-02 | |
| UI-20 | For an MR scan the View tab offers Auto, Soft and Hard, as 0.6.0 does. | 7.6 | | |
| UI-21 | Patient details are hidden unless "Show patient details" is turned on; the setting starts off in every run. | 7.12 | R-UI-05 | |
| UI-22 | Rulers, angles, regions and markers can be drawn on the slices as well as on the planes and in 3D, as soon as a slice is shown, without waiting for a 3D volume; measurements kept by 0.6.0 still open. | 7.7, 5 | R-UI-03 | |
| UI-23 | A scan with a pixel spacing but no position is measured within the picture and its measurements say they come from the file's pixel spacing. | 7.7 | R-UI-03 | |
| UI-24 | A scan with no pixel spacing is not measured in millimetres, and the tools say why; a detector's spacing alone is never taken as the body's. | 7.7 | R-UI-03 | |
| UI-25 | Measurement labels drawn over a picture never overlap one another. | 7.7 | R-UI-04 | |
| UI-26 | The Labels tab shows who made the labels and the model's own check before any part, then an overview by group, then the list of parts. | 7.9 | R-UI-08 | |
| UI-27 | The Tools tab lists "Label this brain scan" under "Part of Voxa" with its state, progress and stop, and has a section "Added by you" that the plugin system can fill. | 7.10 | | |
| UI-28 | Notices are in one place at the bottom right: the newest, how many, and a list of all with Dismiss. | 7.11 | | |
| UI-29 | Light and dark themes and the system's choice; the pictures stay on black in both; every pair of text and background meets WCAG AA contrast. | 7.13 | R-UI-09 | |
| UI-30 | The window works down to 640×480: both side columns fold, and the top bar keeps Open, search and the menu. | 7.14 | | |
| UI-31 | On first start the window fits the screen it opens on; F11 enters and leaves full screen; the interface size is chosen from the menu and remembered. | 7.14 | | |
| UI-32 | The rail and the tab panel can be dragged wider or narrower and folded, and their widths and folds are remembered. | 7.1, 7.16 | | |
| UI-33 | Every control, scan card and viewport has a screen-reader name; the hand-drawn ones say what they show. | 7.14 | | |
| UI-34 | A DICOM structured report is listed in Other files and opens as text, headed "Shown as text. The original report is the reference." | 7.15 | R-UI-10 | |
| UI-35 | A PDF stored in DICOM is listed and can be saved as a PDF file where the person chooses; Voxa starts no other program. | 7.15 | | |
| UI-36 | A settings file written by 0.6.0 opens without error and its settings are honoured; 3D views and measurements kept per series are kept. | 7.16 | | |
| UI-37 | Drawing the bars, rail and panel adds no work per frame that grows with the scan's size; with a made-up study of 500 series it takes no longer than 0.6.0's bars and list. | 7.17 | | |
| UI-38 | No wording on screen says or suggests a finding: no "normal", "abnormal", "lesion" or score. | 4 | R-UI-08 | |
| UI-39 | Help topics on opening an export, finding your scan, brightness, measuring and where its millimetres come from, left and right, labels, and what Voxa is not for, are found by search and listed under help. | 7.18 | R-UI-03, R-UI-08 | |
