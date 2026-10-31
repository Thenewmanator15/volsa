# Usability file

How Voxa is made easy and safe to use, and the evidence. Kept in the spirit of IEC 62366-1.
Started with the redesign of the window (design:
`docs/superpowers/specs/2026-10-30-voxa-ui-redesign-design.md`).

Never record anything from a participant's scan here: no names, dates, descriptions,
pictures or counts. Write what the person did and said about Voxa, not about their body.

## 1. Use specification

- **Intended use**: see `docs/intended-use.md`. Looking at scans on your own computer: your
  own, and scans for learning, teaching and research. Never diagnosis.
- **Users**: (1) people viewing their own scans, most with no medical or technical
  training, often anxious about what a scan shows; (2) students; (3) researchers.
- **Where**: a Windows computer at home, in a library or an office; a laptop screen as small
  as 1366×768, often with the interface scaled; a mouse, a touchpad, or the keyboard alone;
  sometimes a screen reader.
- **What they bring**: an export from a hospital (a CD's folder, a zip, a DICOMDIR), which
  may hold several studies, scanned paperwork and reports beside the scans.

## 2. Parts of the window that bear on safety

- Orientation: the letters at each edge, and the radiological or neurological convention.
- Scale: the scale bar, and where measurements get their millimetres from.
- Values: the window, the value under the pointer and its unit.
- Labels: who made them and the model's own check.
- Which scan and whose: the scan's name and description; patient details when shown.
- The disclaimer.

The risk register (`risk-register.md`) holds what could go wrong with each: orientation in
R-UI-13 and R-UI-09, scale in R-UI-03, values in R-UI-02, labels in R-UI-08, R-UI-12 and
R-UI-14, which scan and whose in R-UI-01 and R-UI-05, and the disclaimer in R-UI-15.

## 3. Problems seen in 0.6.0

From captures of 0.6.0 with made-up studies (2026-10-27, 2026-10-28 and 2026-10-30):

- U-01. About forty controls in three rows across the top, not grouped by task.
- U-02. Scanned paperwork listed, and opened, before the scans.
- U-03. The measurements window opens over the list of series.
- U-04. Warnings such as "! several planes" sit in the list of series.
- U-05. The list of labelled parts is long, with no overview.
- U-06. Opens on the 3D view; the slices are in a narrow column.
- U-07. Measurement labels in 3D drawn over one another.
- U-08. In an 800×600 window the bar wraps onto four lines.
- U-09. Measuring only on the planes and in 3D, not on the slices a newcomer sees first.

## 4. Use scenarios to test

- S-01. Open an export and find the brain scan. *(success test, part 1)*
- S-02. Measure a length on it. *(success test, part 2)*
- S-03. Make the picture brighter or darker.
- S-04. Find the report among the other files and read it.
- S-05. Tell the patient's left from their right.
- S-06. Label a brain scan with Voxa, and say who made the labels.
- S-07. Do S-01 and S-02 with the keyboard alone.

## 5. Test script

For each participant, on their own computer, with Voxa's zip and either their own export or
a made-up study handed to them as a folder (written beforehand with
`cargo run -p voxa-app --example write_phantom`). That study is a ball: it has **no brain scan**
(D-08), and `write_phantom head` needs a model folder this work does not have. So with the made-up
study, step 3 asks for the finer of the ball's scans, as it is named in the rail ("ball, fine");
it is a weaker test of finding "the brain scan" among others, since the scans differ in their
descriptions only. For the test as designed, the study handed over must hold a brain scan beside
others (a mode of `write_phantom` that writes one is not made yet); until it is, the formative
sessions use participants' own exports.

1. Say: "This is a viewer for scans. Please think aloud. I can't help, but there are no
   wrong answers: if something is hard, that is Voxa's fault."
2. Ask them to open the export (S-01). Start a timer when Voxa's window first shows.
3. "Find the scan of the brain." (With the made-up ball: "Find the scan called ball, fine.")
   Note the time, and every place they looked first.
4. "Measure something on it: anything you like." Note the time, and how they found the tool.
5. Then S-03 to S-06 as time allows; S-07 for a keyboard user.
6. Ask: what was hardest; what they expected to find and did not; what they thought any
   number or label meant.
7. Record below: the date, the participant as "P1", "P2" (kind only: own scan, student,
   researcher), the version, each task's time and outcome (done unaided, done with a hint,
   not done), and what they did and said. Nothing from their scan.

**Pass**: the success test (S-01 and S-02) done unaided within a minute.

## 6. Results

### Formative: 0.6.0 preview (*owner*, 2026-10-30)

Three to five people, during the redesign. None recorded yet.

### Recorded check: the window's own work per frame (2026-10-31)

Spec 7.17 holds the cost of drawing the window's bars, rail and panel to what 0.6.0's bars and
list of series cost, and says it stays within that with a made-up study of 500 series.

- **What was timed.** One frame of the window laid out headless, with no screen and no
  pictures, at 1400 by 900 points, for a made-up study of 500 series of one slice each
  (`crates/voxa-app/examples/measure_window.rs`: 10 frames to warm up, then 200 timed, each
  with the drawing turned into triangles). The redesign's frame is the top bar, the bottom
  line, the rail and the tab panel on its View tab; 0.6.0's is its toolbar and status line and
  its list of series. Both are the library's own code laid out by a `BareWindow`, the
  redesign's in `crates/voxa-app/src/ui/bare.rs`, 0.6.0's a temporary copy of the same
  interface over its `bars::show` and `browser::show`, which was never committed and is not
  kept, so the 0.6.0 figures cannot be run again from this repository (0.6.0 has no
  `BareWindow`; the example's own header says so).
- **How.** Release builds. 0.6.0 is commit `ddc5686`, the commit this branch started from, in a
  temporary worktree (removed afterwards), not the `release` branch, which has moved. Nothing
  else heavy was running, as far as was checked; a first pair of runs, made while other work was
  running, was discarded. The two programs were run alternately, five runs each, the 0.6.0 one
  first of each pair.
- **Result.** Median frame time, five runs each:

| | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 |
|---|---|---|---|---|---|
| 0.6.0 (bars and list of series) | 1.184 ms | 1.178 ms | 1.144 ms | 1.158 ms | 1.188 ms |
| Redesign (bars, rail and panel) | 0.202 ms | 0.211 ms | 0.231 ms | 0.199 ms | 0.202 ms |

  The redesign's frame took about a sixth of the time of 0.6.0's with the same 500 series:
  the rail draws only the cards in sight, where 0.6.0's list drew all 500. UI-37 is met.
- **What it does not show.** The pictures, the graphics device and the time to paint are not
  in it. A computer with a slower processor will show larger numbers for both; only the two
  beside each other mean anything.

### Dry run by the builder, not a participant (2026-10-31)

**What this is, and is not.** It is the builder walking the success test (S-01, S-02, and S-07 by
keyboard) as a first-time user would, to find what would trip someone before the formative
sessions. The builder is a program that cannot use a mouse on the real window and knows what every
control does. So this is **not** a test with a person: it gives no time for the task as if a person
had done it, and it **does not claim that the success test is passed**. That waits for the
formative sessions with people (section 5) and the summative one.

**How.** Two ways, step by step. Pictures of the real window were taken with Voxa's capture words
(`VOXA_CAPTURE`, `VOXA_CAPTURE_SHOW`: what is shown is chosen by the words, not by a click) at
1400×900, and Voxa's own headless interface tests, which press the same buttons and keys the
window offers, were run for what lies between the pictures. On two made-up studies, neither
anyone's scan:

- the phantom written by `write_phantom`: three scans of a ball ("ball, coarse", "ball, fine",
  "ball, two echoes") and two other files. It has **no brain scan**, so step 3 of the test script
  ("Find the scan of the brain") cannot be run on it as written, although the script allows a
  made-up study from `write_phantom` to be handed to a participant (D-08);
- a made-up export with brain-like descriptions, written for this dry run by a throwaway program
  that is not kept in the repository: two brain series described in the scanner abbreviations
  common to many makes (a sagittal 3D T1 and an axial T2 FLAIR, each with the body part BRAIN), a
  sagittal T2 of the neck (body part CSPINE) and one report, in one study. None of it comes from
  a real person's scan.

**S-01. Open an export and find the brain scan.**

| Step | What is on the screen, and what a person does | Shown by |
|---|---|---|
| 1 | The window with nothing open: **Open** at the top left; at the right of the top bar the search box, **?**, the theme and **⋯**; the rail headed **Scans** saying, in small grey type, "Open a scan export to begin: a folder, a zip file or a DICOMDIR."; the middle black and saying "No series is shown here"; the panel at the right on **View**, with the brightness boxes W 1.0 and L 0.0 and the other View controls. | capture of the empty window; `ui::rail::tests::with_nothing_open_the_rail_asks_for_an_export` |
| 2 | Click **Open**, then **Folder…** (or **Zip file or DICOMDIR…**), then choose in the system's dialog: three actions. Dropping the export on the window also opens it. | `ui::top::tests::the_open_menu_offers_a_folder_and_a_zip_file_or_dicomdir`. **Not run:** the system's dialog, and dropping a file; the pictures were opened from the command line. |
| 3 | The window opens on **Slices** with one viewport, on the scan Voxa judges best. On the made-up brain export that is the first card, "Brain, 3D T1, a…", selected and shown at once on slice 13 of 24, with the study's name and date in the top bar; the other scans are the cards under it ("Brain, T2 FLAIR…", "Cspine, T2, axial"), and **Other files**, closed, holds the report. On the phantom it opens on "ball, fine". No further click is needed to see the brain scan here. | captures of both studies at 1400×900; `reducer::pictures_tests::an_export_opens_on_slices_with_the_remembered_viewports`, `ui::rail::tests::scans_come_before_other_files_which_start_closed` |
| 4 | Were the scan shown not the one wanted: one click on another card shows it in the active viewport. | `ui::rail::tests::clicking_a_card_shows_that_series_in_the_active_viewport`, `ui::rail::card_tests::a_card_shows_the_plain_name_and_the_description_beneath` |

**S-02. Measure a length on it.**

| Step | What is on the screen, and what a person does | Shown by |
|---|---|---|
| 1 | Find the tool: the panel's tabs read **View, Measure, 3D, Labels, Tools**. One click on **Measure**, or **M**, or the search box (Ctrl+K, then "ruler", "length" or "distance", then Enter). "size" was found in the dry run to offer "Draw the interface at its usual size" before the ruler (D-09, below). | `ui::tabs::measure::tests::m_opens_the_measure_tab_and_unfolds_the_panel`, `ui::search::tests::every_command_is_found_by_its_name_and_each_of_its_words`, `ui::commands::tests::ctrl_k_and_a_slash_open_the_search_and_nothing_else_does` |
| 2 | The Measure tab with nothing measured: **Tools** (Move, Ruler, Angle, Region, Marker), Undo and Redo, "On this series", "Nothing has been measured on this series yet.", and Save as CSV…, Save as JSON… and Clear all greyed. Move is the tool in hand. | capture of the tab on the made-up brain export; `ui::tabs::measure::tests::clicking_down_the_tab_finds_everything_it_offers` |
| 3 | One click on **Ruler**; its button turns blue. The sentence that says how to draw ("Press at one end and let go at the other") was in its hover text only (D-04); since the final review the tab says it under the tools while a tool is in hand, with "Esc puts it down." | `ui::tabs::measure::tests::the_tab_draws_its_buttons_from_the_list` |
| 4 | Press at one end of the length on the slice, drag, let go at the other end. It works on the slice at once, without waiting for a 3D volume. | `ui::viewport::tools_tests::a_ruler_is_drawn_on_a_slice_from_where_it_is_pressed_to_where_it_is_let_go`, `reducer::annotations_tests::a_ruler_can_be_drawn_on_a_slice_before_the_volume_is_ready`. **Not run:** a real mouse. |
| 5 | The result: "Ruler 1: 38.4 mm" is written beside the line on the picture, and the tab lists "Ruler 1", "38.4 mm" and "Its ends can be placed to about one voxel: ± 0.6 mm along this line." | capture of the phantom's slice with the capture word `measured`, which makes one of each kind of measurement (the ruler there was not drawn by a pointer) |

From an open window the measuring takes two clicks and one drag. Counted from nothing open, with
the brain scan the one Voxa opens on, the three actions of S-01 step 2 come first.

**S-07. The same by keyboard alone.**

- Tab goes through the top bar, the rail, the pictures and the panel in that order; the arrow keys
  walk the cards of the rail and Enter opens one
  (`ui::access_tests::rail_keys::tab_goes_through_the_top_bar_the_rail_the_pictures_and_the_panel_in_that_order`,
  `ui::access_tests::rail_keys::the_rail_is_walked_with_the_arrow_keys` and
  `ui::access_tests::rail_keys::enter_opens_the_card_that_has_the_keyboard`). Search opens with
  Ctrl+K or `/`, the arrows move among the results and Enter runs one
  (`ui::search::keys_tests::arrows_move_and_enter_runs_what_is_chosen`). **M** shows the Measure
  tab. So S-01, and choosing the Ruler, can be done from the keyboard, as far as the tests reach (the system's folder dialog was not run by keyboard).
- **Placing the ruler cannot.** No key places or moves the end of a measurement: the keys the
  pictures take step the slices and change brightness, orientation, fit and layout (searched for
  in the window's code, not tried). So S-02 cannot be done by keyboard alone, and S-07 as written
  cannot pass for S-02 whatever the participant does (D-07).
- **Not run:** the system's folder dialog by keyboard, and a screen reader. The names it would read
  are tested (UI-33), not listened to.

**What would trip a first-time user.** Each is for the formative sessions to confirm or drop.

- D-01. The empty window says its one instruction in small grey type in the rail. The large black
  middle says only "No series is shown here", and the View tab shows what look like live controls
  (W 1.0, L 0.0, Auto) for nothing.
- D-02. At 800×600 and smaller the rail is folded to a strip of one list icon and a **»**. A person
  looking for "the brain scan" sees the open scan and no cards, and may not take the strip for the
  list of scans.
- D-03. In a 1400-wide window a card's plain name is cut with an ellipsis ("Brain, 3D T1, a…",
  "Brain, T2 FLAIR…"); the whole is in its hover text. What tells two brain scans apart is the cut
  part. A body part the scanner wrote in one word shows as it was written, with a capital first
  letter: "Cspine, T2, axial" reads like a typing mistake.
- D-04. After **Ruler** is chosen nothing on the screen says how to use it, and the pointer does
  not change (searched for in the window's code: no cursor is set for the tools). A ruler let go
  within a millimetre of where it was pressed is taken for a click and adds nothing (in the
  planes' drawing code, which the slices share, read and not tried), so two clicks, a natural way
  to try, may give no ruler and no word.
- D-05. The "2 files skipped" notice shows on the first screen of the phantom, which holds its
  made-up labels in the study's folder. It is information, but "skipped" can read as scans lost.
  Its list names the files.
- D-06. With a report or a PDF open, the View tab beside it still offers brightness, orientation,
  playing through slices and picture controls, none of which acts on a document. A date inside a
  report shows as the file writes it ("Written on: 20200102"), under a header that says
  "2 January 2020".
- D-07. S-07 cannot be completed (above). The script has a keyboard user attempt S-02, which
  cannot succeed until a key places a measurement, or the scenario is reworded.
- D-08. The script's step 3 needs a study with a brain scan among other scans, and `write_phantom`
  writes none. The made-up export used here is not in the repository.
- D-09. Someone who types "size" into the search to measure a size is offered "Draw the interface
  at its usual size" first, and Enter resets the interface's size. (Found by reading the ranking.)

**After the dry run.** The final review of the branch fixed what it could of these, test first:
D-01 (the empty window says how to begin in its middle, with the buttons that open something);
D-02 (the folded rail says "Scans" under its icon); D-03 (a body part written as one code, such
as "Cspine", is spelled out; the plain names stay cut in a narrow rail and whole in the hover
text); D-04 (a tool in hand says how to draw; a ruler let go within a millimetre of where it was
pressed was not looked at again); D-05 (the notice reads "2 files are not scans or documents Voxa lists"); D-06 (a date in
a report is said in words; beside a report or with nothing open the controls of the pictures are
greyed with their reason, and the View tab shows one line in place of the window and level fields); D-09 (a word an action is searched by, spelt out, ranks with a word
of its name, so "size" offers the measuring tools first). D-07 (no key places a measurement) and
D-08 (no brain scan to hand in `write_phantom`) are open, and the script of section 5 says how
to run it meanwhile. None of this has been seen with a person.

### R45: the smallest window with the interface at 200%

At 640×480 with the interface size at 200% (so that the window is 320 points of the interface
across), taken with `VOXA_CAPTURE_SIZE=320x240` and `VOXA_CAPTURE_ZOOM=2` on a display at 200%, the
dry run saw the folded panel's strip of tab icons cut off at the bottom (the first three whole, the
fourth cut by the bottom line, the fifth not seen), the strip's edge crossing the bottom line
between "diagnostic" and "use.", and the search box showing only "F…".

**What is true now.** The final review's fix sizes the five icons to the height there is, down to
20 points, so that none runs past the bottom line
(`crates/voxa-app/src/ui/panel/tests.rs::the_icons_of_a_folded_panel_fit_in_a_window_as_short_as_the_smallest_at_twice_the_size`).
A capture of that window with the phantom, taken after the fix and looked at, shows the five icons
whole, the disclaimer whole with no line across it, and the search box showing "Fi…" and its
shortcut. What is still not right there is the measurement labels: with the four measurements of the
phantom on the slice, the region's words cover the scale bar's "20 mm" and touch the angle's
(requirements UI-25, acceptance record, known limitations).

### Summative: the redesign

None recorded yet.
