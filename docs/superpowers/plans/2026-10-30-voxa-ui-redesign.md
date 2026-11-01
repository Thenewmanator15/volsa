# Voxa's redesigned window: implementation plan

> Volsa was earlier called Voxa. This record keeps the name it was written with.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild Voxa's window as layout A (top bar with search, scans rail, pictures in the middle, a tab panel), driven by one list of actions, with MR/CT/basic profiles, measuring on slices, documents, themes and screen-reader names.

**Architecture:** First a table of every action (`ui::commands`) that search, keys, help, hints and accessible names read; then the window is rebuilt piece by piece around the unchanged engine (reading, caching, volumes, GPU drawing, workers). Small additive facts go into voxa-dicom and voxa-core (real pixel spacing, rescale type, documents). Each task leaves the app working and `cargo xtask check` green.

**Tech Stack:** Rust 2024, egui/eframe 0.36 (AccessKit on), wgpu 30, dicom-rs 0.10, serde. No new crates.

**Spec:** `docs/superpowers/specs/2026-10-30-voxa-ui-redesign-design.md` (approved 2026-10-30). Requirements `docs/quality/requirements.md` (UI-01 to UI-39), risks `docs/quality/risk-register.md` (R-UI-01 to R-UI-10), usability `docs/quality/usability.md`. Read the spec section each task names.

## Global Constraints

- Gate: `cargo xtask check` green before every commit (rustfmt; clippy pedantic as errors; files ≤ 400 lines, functions ≤ 60 lines; all tests; `cargo deny`; rustdoc `-D warnings`). Split files early: rustfmt's narrow width inflates line counts.
- No new third-party crate. No `unsafe`. Library crates (voxa-core, voxa-dicom, voxa-volume) never panic.
- Never anything from a real export in code, tests, docs or commit messages. Made-up descriptions only, built from the `testing` helpers or `voxa_dicom::testkit`; examples use `write_phantom`.
- Disclaimer word for word: `Voxa is not a medical device and is not for diagnostic use.`
- CT windows (width/level, HU): brain 80/40, soft tissue 400/40, lung 1500/−600, bone 1800/400.
- Smallest window 640×480 (`prefs::SMALLEST_WINDOW`, unchanged).
- Search opens with Ctrl+K, `/` or a click; Esc closes it.
- Patient details start off in every run and are not remembered.
- Exact copy fixed by the spec: `This export holds scans of {n} people.`; `Person 1`…; `from the file's pixel spacing`; `Shown as text. The original report is the reference.`; `Added by you, not part of Voxa`; `Save a copy as PDF…`.
- egui 0.36 is newer than most training data: read its source under `~/.cargo/registry/src` before using an API not already used in Voxa.
- Commit messages end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Never push, never merge into `main` or `release`.
- Each task updates the "Shown by" column of `docs/quality/requirements.md` for the requirements it meets, naming the tests.

## Review Focus

1. Typing in the search box or a measurement's name field must not fire single-letter keys (`H`, `V`, `R`, `F`, `1`, `2`, `4`, `B`, `M`, `/`, `[`, `]`): keys act only while no text field has focus. Test in Task 4.
2. An export with no scans at all (only reports, pages or unreadable files) opens with Other files expanded and the first of them shown, never an empty window that says nothing. Test in Task 11.
3. A very long scanner description or plain name in a narrow rail is cut with an ellipsis, whole in the hover text, and never widens the rail or overlaps the next card. Test in Task 11.
4. Changing the shown scan (a card click, search, `[`/`]`) while a measurement is half-drawn drops the draft; nothing is added to the newly shown series. Test in Task 13.
5. A 0.6.0 settings file with the column dragged to its widest, opened in a 640×480 window, still leaves the pictures at least 200 points wide: the panel and rail fold before the pictures shrink further. Test in Task 9.

---

## Phase 1: foundations (no visible change)

### Task 1: Real pixel spacing and rescale type in the index

Spec 9.2. Requirements UI-19, UI-23, UI-24 (data for them). Risks R-UI-02, R-UI-03.

**Files:**
- Modify: `crates/voxa-core/src/model.rs` (`SeriesInfo`, its `Default`)
- Modify: `crates/voxa-dicom/src/header.rs` (read Rescale Type)
- Modify: `crates/voxa-dicom/src/index.rs:286-300` (fill the two fields)
- Modify: `crates/voxa-dicom/src/testkit.rs`, `crates/voxa-dicom/src/testkit/build.rs` (write them)
- Test: `crates/voxa-dicom/src/index/tests.rs`, `crates/voxa-dicom/src/header.rs` tests

**Interfaces:**
- Produces: `SeriesInfo::spacing_given: bool` (true only when the first image's Pixel Spacing was in the file; `spacing_mm` still holds the 1.0 fallback for drawing), `SeriesInfo::rescale_type: Option<String>` (trimmed text of (0028,1054), top level or in the shared/per-frame Pixel Value Transformation Sequence (0040,9096)); `Header::rescale_type: Option<String>`; `SliceOptions::rescale_type: Option<String>`, `SliceOptions::imager_pixel_spacing: Option<[f64; 2]>` (written as (0018,1164)).

- [ ] **Step 1: Write the failing tests** in `index/tests.rs`:

```rust
#[test]
fn a_series_without_pixel_spacing_says_its_spacing_was_not_given() {
    // made-up stack, pixel_spacing: None
    let index = /* one series from SliceOptions { pixel_spacing: None, .. } */;
    let info = &index.series[0].info;
    assert!(!info.spacing_given);
    assert_eq!(info.spacing_mm[..2], [1.0, 1.0]); // drawing fallback unchanged
}
#[test]
fn a_detectors_spacing_alone_is_not_taken_as_the_bodys() {
    // pixel_spacing: None, imager_pixel_spacing: Some([0.2, 0.2])
    assert!(!info.spacing_given);
}
#[test]
fn a_series_with_pixel_spacing_says_so() { assert!(info.spacing_given); }
#[test]
fn the_rescale_type_is_read_and_trimmed() {
    // rescale_type: Some("HU ".into()) -> Some("HU"); None -> None
}
```

- [ ] **Step 2: Run** `cargo test -p voxa-dicom spacing rescale` — expect FAIL (fields missing).
- [ ] **Step 3: Implement** the fields; header reads (0028,1054) with the same top-level-then-functional-group pattern `of_the_plane` uses; index sets `spacing_given: first.pixel_spacing.is_some()`, `rescale_type: first.rescale_type.clone()`. Never read (0018,1164) into `pixel_spacing`.
- [ ] **Step 4: Run** `cargo test -p voxa-dicom -p voxa-core` — PASS; fix any test constructors of `SeriesInfo` across the workspace (`cargo test --workspace --no-run`).
- [ ] **Step 5: Commit** `feat(dicom): the index says whether pixel spacing was given, and reads the rescale type`

### Task 2: Profiles: CT, MR, basic

Spec 7.6. Requirements UI-17, UI-18, UI-19, UI-20. Risk R-UI-02.

**Files:**
- Create: `crates/voxa-app/src/profile.rs`, `crates/voxa-app/src/profile/tests.rs`
- Modify: `crates/voxa-app/src/lib.rs` (module)

**Interfaces:**
- Consumes: Task 1's `SeriesInfo::rescale_type`.
- Produces:
  - `pub enum Profile { Basic, Mr, Ct }` with `pub fn of(info: &SeriesInfo) -> Profile` (Modality, trimmed and upper-cased: `"CT"` → Ct, `"MR"` → Mr, else Basic).
  - `pub struct NamedWindow { pub name: &'static str, pub wl: WindowLevel }` and `pub const CT_WINDOWS: [NamedWindow; 4]` in the order Brain, Soft tissue, Lung, Bone.
  - `pub enum Unit { Hu, None, Named(String) }`, `pub fn unit_of(info: &SeriesInfo) -> Unit`: Ct with rescale type `None` or `"HU"` → Hu; any profile with `"US"` (DICOM's "unspecified") or none → None; another rescale type → `Named(code)`; a non-CT `"HU"` → `Named("HU")`.
  - `pub fn readout(value: f32, unit: &Unit) -> String`: `"-12 HU"`, `"312 (no unit)"`, `"0.4 OD"`; whole numbers for HU, three significant figures otherwise.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn the_profile_comes_from_the_modality() {
    assert_eq!(Profile::of(&with_modality("CT")), Profile::Ct);
    assert_eq!(Profile::of(&with_modality("mr ")), Profile::Mr);
    for other in ["US", "CR", "PT", "SC", ""] { assert_eq!(Profile::of(&with_modality(other)), Profile::Basic); }
}
#[test] fn ct_windows_are_the_common_teaching_values() {
    let got: Vec<_> = CT_WINDOWS.iter().map(|w| (w.name, w.wl.width, w.wl.center)).collect();
    assert_eq!(got, [("Brain", 80.0, 40.0), ("Soft tissue", 400.0, 40.0), ("Lung", 1500.0, -600.0), ("Bone", 1800.0, 400.0)]);
}
#[test] fn hu_only_for_ct_whose_rescale_says_hu_or_nothing() {
    assert_eq!(unit_of(&ct(None)), Unit::Hu);
    assert_eq!(unit_of(&ct(Some("HU"))), Unit::Hu);
    assert_eq!(unit_of(&ct(Some("US"))), Unit::None);
    assert_eq!(unit_of(&ct(Some("MGML"))), Unit::Named("MGML".into()));
    assert_eq!(unit_of(&mr(None)), Unit::None);
}
#[test] fn readouts_say_their_unit() {
    assert_eq!(readout(-12.4, &Unit::Hu), "-12 HU");
    assert_eq!(readout(312.0, &Unit::None), "312 (no unit)");
}
```

- [ ] **Step 2: Run** `cargo test -p voxa-app profile` — FAIL.
- [ ] **Step 3: Implement** in `profile.rs` (check `WindowLevel::new(center, width)` argument order in voxa-core before writing the table).
- [ ] **Step 4: Run** — PASS. Fill "Shown by" for UI-17, UI-18.
- [ ] **Step 5: Commit** `feat(app): a profile for each scan from its modality, with CT's named windows and units`

### Task 3: Plain names and the two groups of files

Spec 7.4, 9.1. Requirements UI-09, UI-10, UI-11. Risk R-UI-01.

**Files:**
- Create: `crates/voxa-app/src/naming.rs`, `crates/voxa-app/src/naming/words.rs` (vocabulary), `crates/voxa-app/src/naming/tests.rs`

**Interfaces:**
- Produces:
  - `pub fn plain_name(info: &SeriesInfo) -> String`
  - `pub enum Group { Scans, Other }`, `pub fn group_of(info: &SeriesInfo) -> Group`: Other for `SeriesKind::Locating` and for Modality `SC`, `OT`, `DOC`; Scans for everything else, including unreadable series and colour series of an imaging modality.
- Rules (the vocabulary itself is the builder's, these are pinned):
  - Split the description into words on anything not a letter or digit (keep `T2*` as `T2*`); match case-insensitively against the vocabulary. Only vocabulary words are carried into the name; a word not in the vocabulary never appears.
  - Weighting words (`T1`, `T2`, `PD`, `FLAIR`, `T2*`, `DWI`/`TRACE`/`DIFFUSION` → `Diffusion`, `ADC` → `ADC map`, `SWI`/`SWAN` → `SWI`, `STIR`, `TOF` → `Angiography (TOF)`, `MPRAGE`/`MP-RAGE`/`BRAVO` → `3D T1`). If two of `T1`, `T2`, `PD` appear, no weighting is named.
  - Body part from `info.scan.body_part` only (`HEAD` → `Head`), never from the description.
  - Plane from `info.scan.plane` (geometry) first, else from the description (`AX`/`AXIAL`/`TRA` → axial, `SAG` → sagittal, `COR` → coronal); `3D` from `info.scan.in_3d == Some(true)` or the word `3D`.
  - MR with a weighting: `"{Body}, {weightings}{, 3D}{, plane}"` (body omitted when unknown). MR without: `"{Plane} scan{, 3D}"` prefixed `"{Body}, "` when known; no plane and no body → `"Scan {number}"`.
  - Other modalities: `"{Body} {Modality words}{, plane}"` with CT → `CT`, CR/DX → `X-ray`, US → `Ultrasound`, PT → `PET`, NM → `Nuclear medicine`, MG → `Mammogram`, XA → `X-ray angiography`; unknown modality → its code.

- [ ] **Step 1: Write the failing tests** (all descriptions made up):

```rust
#[test] fn known_words_become_a_plain_name() {
    assert_eq!(plain_name(&mr("sag_mprage_demo", Some(Sagittal), Some(true), "HEAD")), "Head, 3D T1, sagittal");
    assert_eq!(plain_name(&mr("ax_t2_demo", Some(Axial), Some(false), "")), "T2, axial");
}
#[test] fn conflicting_weightings_name_none() {
    assert_eq!(plain_name(&mr("demo t1 t2 compare", Some(Axial), None, "")), "Axial scan");
}
#[test] fn with_nothing_known_the_name_is_facts() {
    assert_eq!(plain_name(&mr("", Some(Coronal), Some(true), "")), "Coronal scan, 3D");
    assert_eq!(plain_name(&mr("xyz123", None, None, "")), "Scan 7"); // number 7
}
#[test] fn the_body_part_comes_only_from_its_element() {
    assert_eq!(plain_name(&mr("brain t1 post", Some(Axial), None, "")), "T1, axial");
}
#[test] fn unknown_words_never_reach_the_name() {
    let name = plain_name(&mr("t2 zzqq secretname", Some(Axial), None, ""));
    assert!(!name.to_lowercase().contains("zzqq") && !name.to_lowercase().contains("secretname"));
}
#[test] fn other_modalities_say_what_they_are() {
    assert_eq!(plain_name(&with("CT", "", Some(Axial), "HEAD")), "Head CT, axial");
    assert_eq!(plain_name(&with("DX", "", None, "CHEST")), "Chest X-ray");
}
#[test] fn pages_and_locating_images_are_other_files() {
    assert_eq!(group_of(&with("SC", "", None, "")), Group::Other);
    assert_eq!(group_of(&locating()), Group::Other);
    assert_eq!(group_of(&unreadable_mr()), Group::Scans);
    assert_eq!(group_of(&colour_us()), Group::Scans);
}
```

- [ ] **Step 2: Run** `cargo test -p voxa-app naming` — FAIL.
- [ ] **Step 3: Implement.** Keep `words.rs` a plain table of `(&str, &str)` pairs and plane words.
- [ ] **Step 4: Run** — PASS. Fill "Shown by" for UI-11 and R-UI-01's check.
- [ ] **Step 5: Commit** `feat(app): plain names from the scanner's own words, and which files are scans`

### Task 4: The list of actions, with keys and the keys' help read from it

Spec 7.2. Requirements UI-03, UI-04. Review Focus 1.

**Files:**
- Create: `crates/voxa-app/src/ui/commands.rs` (types, lookups, `button`), `crates/voxa-app/src/ui/commands/table.rs` (the entries, split into `table/slices.rs`, `table/scene.rs`, `table/measure.rs`, `table/window.rs` to stay under 400 lines), `crates/voxa-app/src/ui/commands/tests.rs`
- Create: `crates/voxa-app/src/ui/requests.rs` (`impl VoxaApp { fn ask(&mut self, ctx, request) }`)
- Modify: `crates/voxa-app/src/ui/keys.rs` (use `commands::for_key`), `crates/voxa-app/src/ui/help.rs` (keys list from the table), `crates/voxa-app/src/ui/overlay.rs` (remove `SHORTCUTS` and `shortcut_action`), `crates/voxa-app/src/ui/toolbar.rs`, `crates/voxa-app/src/ui/scene_bar.rs` (buttons drawn through `commands::button`)

**Interfaces:**
- Produces (used by every later UI task):

```rust
pub struct Command {
    pub id: &'static str,                 // "slice.next", "measure.ruler", ...
    pub name: &'static str,               // as on its button
    pub words: &'static [&'static str],   // other search words
    pub key: Option<Shortcut>,
    pub place: Place,
    pub profiles: &'static [Profile],     // empty: every profile
    pub why_not: fn(&AppState) -> Option<&'static str>,
    pub does: Does,
}
pub struct Shortcut { pub key: egui::Key, pub command: bool, pub shift: bool, pub shown: &'static str }
pub enum Place { TopBar, Menu, Strip, Tab(Tab), Card, Keys }
pub enum Does { Act(fn(&AppState) -> Option<Action>), Ask(Request) }
pub enum Request { Open(Pick), Search, Help(Option<&'static str>), About, Tags, FullScreen,
                   ShowTab(Tab), FoldRail, FoldPanel, Theme(ThemeChoice), Size(f32),
                   SaveMeasurements(Save), OpenLabels, ShowOtherFiles }
pub fn all() -> &'static [Command];
pub fn find(id: &str) -> Option<&'static Command>;
pub fn for_key(key: egui::Key, held: Held, state: &AppState) -> Option<&'static Command>;
pub fn hint(command: &Command) -> String;   // "Name (Key)" or "Name"
pub fn button(ui: &mut egui::Ui, id: &str, state: &AppState, selected: Option<bool>) -> Option<Does>;
```

  `Tab` is defined here for now (`pub enum Tab { View, Measure, Scene, Labels, Tools }`) and moved to `ui/panel.rs` in Task 6 with a re-export. `ThemeChoice` is a placeholder enum `{ System, Light, Dark }` in `ui/theme.rs`, wired in Task 17. `button` draws the label, adds `hint`, disables with `why_not` as the disabled hover text, and sets the accessible name to `name`.
- Every action 0.6.0 offers by key or button gets an entry (the 20 rows of today's `SHORTCUTS`, the toolbar and scene bar buttons). Per-viewport actions read `state.active` in their `Act` function.

- [ ] **Step 1: Write the failing tests** in `commands/tests.rs`:

```rust
#[test] fn ids_are_unique() { /* collect ids into a set, len equals all().len() */ }
#[test] fn no_two_commands_share_a_key() { /* (key, command, shift) unique over entries with a key */ }
#[test] fn every_key_of_0_6_0_still_does_what_it_did() {
    // H, V, R, F, 1, 2, 4, Space, ArrowUp/Down, PageUp/Down, Home, End, [, ], M, Ctrl+Z, Ctrl+Y,
    // Ctrl+Shift+Z, Esc (with a tool in hand), B, F11, ? : for_key(..) finds a command whose
    // does, run on thick_scan_ready(), gives the same Action 0.6.0's shortcut_action gave.
}
#[test] fn the_help_lists_every_command_with_a_key() { /* help::keys_listed() == commands with key */ }
#[test] fn every_command_has_a_name_and_one_other_word() { /* name non-empty; words non-empty */ }
#[test] fn keys_do_nothing_while_a_text_field_has_focus() {
    // headless ctx: a TextEdit focused; press H, 1, /, M through VoxaApp's key path (keys::pressed
    // wrapper taking ctx): no action and no request.
}
```

- [ ] **Step 2: Run** `cargo test -p voxa-app commands` — FAIL.
- [ ] **Step 3: Implement** the table and lookups; port `keys::pressed` to `for_key`; `help.rs` renders `all()` entries with keys plus today's `MOUSE` rows; `requests.rs` holds what today's `Asked` flags do (about, help, tags, measure, list, size, open). Toolbar and scene bar draw their buttons through `commands::button` so their words and hints come from the table.
- [ ] **Step 4: Run** `cargo test -p voxa-app` — PASS (the existing toolbar and scene-bar sweep tests still pass unchanged).
- [ ] **Step 5: Capture check:** `VOXA_CAPTURE=...png VOXA_CAPTURE_SIZE=1400x900 target/release/voxa.exe <phantom>` and compare with `now-1400.png`: same window. Fill "Shown by" for UI-03, UI-04.
- [ ] **Step 6: Commit** `refactor(app): one list of actions that the buttons, keys and help read`

### Task 5: Search

Spec 7.3. Requirements UI-05, UI-06, UI-07, UI-08. Risk R-UI-07.

**Files:**
- Create: `crates/voxa-app/src/ui/search.rs` (the box and its list), `crates/voxa-app/src/ui/search/matching.rs` (pure), `crates/voxa-app/src/ui/search/tests.rs`
- Modify: `crates/voxa-app/src/ui/mod.rs` (a `search: search::Search` field; shown each frame), `crates/voxa-app/src/ui/commands/table/*.rs` (the `search.open` command: Ctrl+K and `/`)

**Interfaces:**
- Consumes: `commands::{all, Command, Does}`; `state.labels` for parts; `state.index` for scans; `naming::plain_name`; help topic ids (Task 19 adds them; until then the Help heading is empty).
- Produces:

```rust
pub enum Heading { Actions, Parts, Scans, Help }
pub enum Target { Command(&'static str), Part(usize), Scan(SeriesId), Help(&'static str) }
pub struct Entry { pub heading: Heading, pub name: String, pub words: Vec<String>, pub target: Target }
pub fn fold(text: &str) -> String;                       // lower case, Latin accents removed
pub fn score(query: &str, name: &str, words: &[String]) -> Option<u32>;
pub fn ranked(query: &str, entries: &[Entry]) -> Vec<usize>;   // best first, at most 8 per heading
#[derive(Default)] pub struct Search { /* open, query, chosen, entries, built_for: u64 */ }
impl Search { pub fn show(&mut self, ctx: &egui::Context, state: &AppState) -> Option<Target>; pub fn open(&mut self); }
```

- Scoring, highest first: the whole name equals the query; the name starts with it; a word of the name starts with it; one of `words` starts with it; the query's letters appear in order in the name (lower the more letters skipped). Each query word must match; the entry's score is its weakest word's. Ties: shorter name, then table order.
- The entries are rebuilt only when a stamp of (the open index's series count, the labels' stamp, the command count) changes, never each frame.
- Running a target: Command → its `Does` (an unavailable command posts `Notice { kind: Info, text: why_not }` instead); Part → `Action::PartSelect(Some(i))` and `Request::ShowTab(Tab::Labels)`; Scan → `Action::SelectSeries { viewport: state.active, series }`; Help → `Request::Help(Some(id))`.
- Commands that throw work away (`measure.clear`, `labels.close`) are entries whose `Does` asks first: a confirmation line inside the search list ("Clear all 3 measurements? Enter again to confirm"), or the action is one undo already covers (`AnnotationsClear` is undoable today: keep it so and say so in its hint).

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn every_command_is_found_by_its_name_and_each_of_its_words() {
    let entries = entries_for(&AppState::default());
    for c in commands::all() {
        for q in std::iter::once(c.name).chain(c.words.iter().copied()) {
            let top = ranked(q, &entries);
            assert!(top.iter().take(8).any(|&i| matches!(entries[i].target, Target::Command(id) if id == c.id)), "{q}");
        }
    }
}
#[test] fn starts_of_words_beat_scattered_letters() { /* "rul" ranks "Measure a length" (word "ruler") above a name containing r..u..l scattered */ }
#[test] fn case_and_accents_are_ignored() { assert_eq!(fold("Éé Ü"), "ee u"); }
#[test] fn parts_and_scans_are_found() { /* labelled_head(): "left" finds a part; three_series(): a series' plain name finds it */ }
#[test] fn an_unavailable_command_is_listed_and_says_why() { /* AppState::default(): "ruler" listed; running it gives Action::Post(notice with why_not) */ }
#[test] fn the_list_is_not_rebuilt_while_nothing_changes() { /* show twice with the same state: built_for unchanged, entries pointer-equal */ }
#[test] fn nothing_typed_is_kept() { /* Search has no serde; prefs::Prefs has no field for it (compile-time: grep test on prefs.rs text) */ }
```

- [ ] **Step 2: Run** `cargo test -p voxa-app search` — FAIL.
- [ ] **Step 3: Implement.** Draw as a popup under the top bar's box (Task 10 places the box; until then Ctrl+K opens it at the top centre). Arrow keys move, Enter runs, Esc closes; the query field has focus while open.
- [ ] **Step 4: Run** — PASS. Fill "Shown by" for UI-05 to UI-08.
- [ ] **Step 5: Commit** `feat(app): search finds every action, part, scan and help topic`

## Phase 2: the window

### Task 6: The tab panel, with the View and 3D tabs

Spec 7.1, 7.6 (controls only; profiles in Task 12), 7.8. Requirements UI-01 (part), UI-32 (panel).

**Files:**
- Create: `crates/voxa-app/src/ui/panel.rs` (`Tab`, frame, fold, drag), `crates/voxa-app/src/ui/tabs.rs`, `crates/voxa-app/src/ui/tabs/view.rs`, `crates/voxa-app/src/ui/tabs/scene.rs`, tests beside each
- Modify: `crates/voxa-app/src/ui/mod.rs`, `crates/voxa-app/src/ui/bars.rs` (drop the scene line), `crates/voxa-app/src/ui/toolbar.rs` (window, switches and playback leave it), `crates/voxa-app/src/ui/screen.rs` and `crates/voxa-app/src/prefs.rs` (panel width, fold, tab)
- Delete: `crates/voxa-app/src/ui/scene_bar.rs` (its contents and tests move to `tabs/scene.rs`)

**Interfaces:**
- Consumes: `commands::button`, `commands::Tab` (moved here, re-exported from `commands`).
- Produces: `pub enum Tab { View, Measure, Scene, Labels, Tools }` with `pub const ALL: [Tab; 5]` and `pub fn title(self) -> &'static str` ("View", "Measure", "3D", "Labels", "Tools"); `ui::panel::show(ui, &mut PanelState, state, |ui, tab| ...)`; `pub struct PanelState { pub tab: Tab, pub folded: bool, pub share: f32 }`; `prefs::Prefs` gains `panel_share: Option<f32>`, `panel_folded: bool`, `tab: String` (all `#[serde(default)]`).
- Panel width: at least 260 points, at most 40% of the window; folded it is a 44-point strip of the five tab icons, each with its accessible name.
- Tabs Measure, Labels and Tools show one line saying where their content is today, until Tasks 7 and 8.

- [ ] **Step 1: Write the failing tests:** `the_panel_shows_the_five_tabs_in_order` (titles from a headless pass); `choosing_a_tab_is_remembered` (prefs round trip with `tab: "Scene"`); `patient_details_start_off_and_are_not_remembered` (turned on, prefs written and read back into a fresh window: off; `Prefs` serialises no such field); `a_folded_panel_is_a_strip_of_tab_icons_with_names`; `the_panel_is_held_between_260_points_and_two_fifths` (pure width fn); move `scene_bar`'s five sweep tests to `tabs/scene.rs`, sweeping the tab instead of the line.
- [ ] **Step 2: Run** `cargo test -p voxa-app panel tabs` — FAIL.
- [ ] **Step 3: Implement.** The panel is an `egui::Panel::right`, exact width from `PanelState::share`, with the same `shares::divider` drag the rail uses. The View tab holds, top to bottom: brightness (today's `toolbar::window`), orientation (Radiological, said "Patient's left on the right"), slices (playback, the volume stepper), and the switches Smooth, Link, Show patient details.
- [ ] **Step 4: Run** `cargo test -p voxa-app` — PASS. Capture at 1400×900 and 800×600: the top now has one line of controls fewer; the panel is at the right.
- [ ] **Step 5: Commit** `feat(app): a tab panel at the right, with the View and 3D tabs`

### Task 7: The Measure tab

Spec 7.7 (the tab), 7.1 (nothing floats over the rail). Requirements UI-01 (part). Usability U-03.

**Files:**
- Create: `crates/voxa-app/src/ui/tabs/measure.rs` (from `ui/measure.rs`'s `contents`, `TOOLS`, saving), tests
- Modify: `crates/voxa-app/src/ui/mod.rs` (no floating Measurements window), `crates/voxa-app/src/ui/commands/table/measure.rs` (`M` → `Request::ShowTab(Tab::Measure)`)
- Delete: `crates/voxa-app/src/ui/measure.rs` once its contents and tests have moved

**Interfaces:**
- Consumes: `panel::Tab::Measure`, `commands::button` for each tool.
- Produces: `tabs::measure::contents(ui, state, &mut MeasureTab, actions) -> Option<Save>`; `pub struct MeasureTab { editing: Option<(u64, String)> }`.

- [ ] **Step 1: Write the failing tests:** move `ui/measure/tests.rs` to `tabs/measure/tests.rs`; add `m_opens_the_measure_tab_and_unfolds_the_panel` and `no_window_opens_over_the_rail` (after pressing M, the pass has no `egui::Window` titled "Measurements").
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement.** **Step 4: Run** `cargo test -p voxa-app` — PASS.
- [ ] **Step 5: Commit** `feat(app): measurements are a tab, not a window over the series`

### Task 8: The Labels and Tools tabs

Spec 7.9, 7.10. Requirements UI-26, UI-27. Risk R-UI-08. Usability U-05.

**Files:**
- Create: `crates/voxa-app/src/ui/tabs/labels.rs` (from `ui/parts.rs`'s `contents`), `crates/voxa-app/src/ui/tabs/labels/overview.rs`, `crates/voxa-app/src/ui/tabs/tools.rs`, `crates/voxa-app/src/tools.rs`, tests beside each
- Modify: `crates/voxa-app/src/ui/parts.rs` (keeps `rows`, `PartsView`; loses the strip and the column drawing), `crates/voxa-app/src/ui/parts/labelling.rs` (strip drawn by the Tools tab), `crates/voxa-app/src/planes.rs` (`ColumnShows::Parts` removed), `crates/voxa-app/src/ui/layout.rs` (no Parts tab in the column header)

**Interfaces:**
- Produces (for the plugin session, documented in `tools.rs`):

```rust
pub enum MadeBy { Voxa, AddedByYou }
pub struct ToolInfo {
    pub name: String, pub made_by: MadeBy, pub source: String, pub licence: String,
    pub checked: Option<bool>, pub accepts: String, pub returns: String,
}
```

  `tabs::tools::contents(ui, state, held, added: &[ToolInfo], actions) -> Option<labelling::Wants>`: section "Part of Voxa" with "Label this brain scan" (today's strip), section "Added by you" listing `added` (empty now) or the line `Tools you add will be listed here. Adding tools comes in a later version of Voxa.`
- `tabs::labels::contents(ui, state, view, actions) -> Option<parts::Asked>`: who made the labels and the model's check first (today's order), then `overview::show` (one row per group: swatch, group name, `n parts`, a show/hide toggle sending `Action::GroupShow`, a click that scrolls the list to the group), then today's searchable list. Without labels: `WHAT_LABELS_ARE`, "Open labels…", and `Label this scan in the Tools tab.` with a button sending `Request::ShowTab(Tab::Tools)`.

- "Label this brain scan" is also a command (`tools.label`, words `segment`, `parts`, `synthseg`) so search finds it; it runs what the strip's button runs.

- [ ] **Step 1: Write the failing tests:** `labelling_is_found_by_search_and_runs_as_the_button_does`; `the_labels_tab_says_who_made_them_before_any_part` (labelled_head: the y of `MADE_ELSEWHERE` < the y of the first part row); `the_overview_has_one_row_per_group_with_its_count`; `hiding_a_group_from_the_overview_sends_group_show`; `the_tools_tab_lists_labelling_under_part_of_voxa`; `added_by_you_is_empty_and_says_so`; move the Parts tests from `ui/parts/tests.rs`.
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement.** **Step 4: Run** `cargo test -p voxa-app` — PASS.
- [ ] **Step 5: Commit** `feat(app): Labels and Tools tabs; the parts list gets an overview by group`

### Task 9: The pictures in four arrangements, and the strip above them

Spec 7.5. Requirements UI-15, UI-16. Review Focus 5. Usability U-06.

**Files:**
- Create: `crates/voxa-app/src/pictures.rs` (the enum), `crates/voxa-app/src/ui/strip.rs`, tests
- Modify: `crates/voxa-app/src/state.rs` (`pictures: Pictures` replaces `main: MainPanel`), `crates/voxa-app/src/action.rs` (`SetPictures(Pictures)` replaces `SetMain` and `SetColumn`), `crates/voxa-app/src/reducer.rs` and `reducer/*.rs` (every `MainPanel`/`ColumnShows` use), `crates/voxa-app/src/scene.rs` (remove `MainPanel`), `crates/voxa-app/src/planes.rs` (remove `ColumnShows`, `Planes::column`), `crates/voxa-app/src/ui/layout.rs` (rewrite `areas`), `crates/voxa-app/src/ui/pictures.rs`, `crates/voxa-app/src/ui/capture.rs` (words `planes`, `quad`, `3d`, `slices3d` set the arrangement), `crates/voxa-app/src/prefs.rs` (carry `column_share` over)

**Interfaces:**
- Produces: `pub enum Pictures { #[default] Slices, SlicesAndScene, Planes, Scene }`; `Pictures::given(self, status: &SceneStatus) -> Pictures` (with no 3D view, every arrangement but Slices gives Slices); `layout::areas(pictures: Pictures, area: Rect) -> Areas` with `Areas { scene: Option<Rect>, planes: Option<[Rect; 3]>, slices: Option<Rect> }` (Slices: slices = area; SlicesAndScene: left half slices, right half scene; Planes: today's `quad`; Scene: scene = area); `strip::show(ui, rect, state, actions)`: the four arrangements, the 1/2/4 buttons while Slices or SlicesAndScene, and at the right `slice {n} of {m}` and the volume of several.
- The strip's buttons are commands (`pictures.slices`, `pictures.slices3d`, `pictures.planes`, `pictures.scene`), so an arrangement the scan cannot have is offered greyed with its `why_not` (`This scan has no 3D view: {reason}`).
- Rules: the 3D scene is drawn only when it has an area; its volume still becomes ready (Ready comes from the worker, not from drawing). The `F` key refits the scene when the arrangement is Scene (today's `MainPanel::Scene` rule). `Esc` with no tool in hand no longer changes the arrangement. Opening an export: arrangement Slices, viewports as remembered (UI-16; `emptied()` keeps the arrangement within a run).
- Prefs: when `panel_share` is absent, `panel_share = column_share × (1 − list_share)`, then held to the panel's limits.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn each_arrangement_gives_its_areas() { /* four cases, rect maths */ }
#[test] fn without_a_3d_view_every_arrangement_shows_slices() { /* given(Unavailable) */ }
#[test] fn an_export_opens_on_slices_with_the_remembered_viewports() {
    /* state with layout TwoByTwo and pictures Planes; reduce OpenPath; then IndexDone with three_series():
       pictures == Slices, layout == TwoByTwo, viewport 0 shows best_series */
}
#[test] fn a_0_6_0_column_width_becomes_the_panels() { /* prefs JSON from 0.6.0 with column_share 0.6, list_share 0.2: panel_share 0.4 clamped */ }
#[test] fn the_pictures_keep_200_points_in_the_smallest_window() {
    /* 640x480, list_share and column_share at their 0.6.0 maxima: laid-out pictures rect width >= 200;
       the rail and panel are folded */
}
```

- [ ] **Step 2: Run** `cargo test -p voxa-app` — FAIL to compile, then FAIL.
- [ ] **Step 3: Implement.** Update every reducer test that used `SetMain`/`SetColumn` to `SetPictures`, keeping what each test checks.
- [ ] **Step 4: Run** `cargo test -p voxa-app` — PASS. Captures with `VOXA_CAPTURE_SHOW` empty, `planes`, `3d`, `slices3d`.
- [ ] **Step 5: Commit** `feat(app): the pictures in four arrangements chosen from a strip; opens on the slices`

### Task 10: The top bar and the notice area

Spec 7.1, 7.11. Requirements UI-01, UI-02, UI-28. Usability U-01, U-08.

**Files:**
- Create: `crates/voxa-app/src/ui/top.rs` (replaces `toolbar.rs`), `crates/voxa-app/src/ui/notices.rs` (replaces `status.rs`), tests
- Modify: `crates/voxa-app/src/ui/bars.rs` (top bar and bottom line only), `crates/voxa-app/src/ui/mod.rs`
- Delete: `crates/voxa-app/src/ui/toolbar.rs`, `crates/voxa-app/src/ui/status.rs` (their tests move)

**Interfaces:**
- Consumes: `commands::button`, `search::Search` (the box), `ThemeChoice`.
- Produces: `top::show(ui, state, search: &mut Search, actions) -> Vec<Request>`; one line: `Open` (a menu: `Folder…`, `Zip file or DICOMDIR…`), the study's name and date (`browser::study_title`), the search box (`Find a tool or part` and `Ctrl K`), Help, the theme button, and `⋯` (interface size, full screen, keys and mouse, where the log is, About). `notices::show(ui, notices, &mut open: bool, actions) -> egui::Response` (the disclaimer's response): at the right, an icon, the newest notice cut to fit (whole on hover), `n notices` when more than one; a click opens a list above the line, newest first, each with Dismiss. Error-kind notices are drawn as Warning (amber); nothing red.

- About, help and a file's tags open as windows over the pictures (their default position inside the pictures' area, never over the rail) and close with Esc.

- [ ] **Step 1: Write the failing tests:** move `bars/tests.rs` (the disclaimer whole at 640×480 at size 2, nothing over it) to the new bars; add `floating_windows_open_over_the_pictures_and_close_with_esc`; `the_top_bar_is_one_line_at_640_wide` (all its widgets share one row's y range at 640×480, size 1); `the_top_bar_keeps_open_search_and_the_menu_at_640`; `the_notice_list_shows_every_notice_newest_first_with_dismiss`; `no_notice_is_drawn_red`.
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement.** **Step 4: Run** `cargo test -p voxa-app` — PASS. Capture at 640×480, 800×600, 1400×900.
- [ ] **Step 5: Commit** `feat(app): one slim top bar with search, and one place for notices`

### Task 11: The scans rail

Spec 7.4. Requirements UI-09, UI-10, UI-12, UI-13, UI-14, UI-37 (rail). Review Focus 2, 3. Risks R-UI-05, R-UI-06. Usability U-02, U-04.

**Files:**
- Create: `crates/voxa-app/src/ui/rail.rs` (grouping, people, Other files, scrolling only what is in sight), `crates/voxa-app/src/ui/rail/card.rs` (from `browser.rs`'s card drawing), `crates/voxa-app/src/ui/rail/people.rs`, tests
- Modify: `crates/voxa-app/src/ui/browser.rs` (keeps `thumbnail`, `caption`, `hover`, `study_title`, `date_in_words`, `one_for_each`, `face_of`; loses `show` and `card`), `crates/voxa-app/src/ui/mod.rs` (`make_thumbnails` skips SC/OT/DOC series while `show_patient` is off), `crates/voxa-app/src/reducer.rs` (`open_best` falls back as below)

**Interfaces:**
- Consumes: `naming::{plain_name, group_of, Group}`, `voxa_core::StudyInfo::{patient_name, patient_id}`.
- Produces: `rail::show(ui, state, thumbnails, &mut RailState, actions) -> Vec<Request>`; `pub struct RailState { pub other_open: bool }`; `people::people_of(index) -> Vec<Person>` grouping studies by patient id (then name) with `Person { label: String, studies: Vec<usize> }`, labels `Person 1`… while patient details are off, the name while on.
- Card: thumbnail (a page icon for Group::Other series of Modality SC/OT/DOC while patient details are off), plain name (strong), the description beneath (small, unchanged; `Series {n}` when empty), the facts line (today's `caption` and `scan.line()`), a mark (`!` in WARNING colour, with an accessible name `has a problem`) for `kind().is_fault()`. Text truncated with an ellipsis; hover shows `hover(info)` with the plain name first.
- Selecting a faulted series posts its `SeriesProblem` sentence as a Warning notice with key `"series"`, replacing the previous one.
- `open_best`: best_series; else the Scans series with the most `rows × cols × slices`; else nothing, and the rail opens Other files (`RailState::other_open = true` when the index has no Scans).
- With more than one person: the line `This export holds scans of {n} people.` at the top.
- A card's menu (right-click, or its `⋯` button, with an accessible name): `Show in the selected view` (`SelectSeries` on the active viewport) and `Show the file's tags` (`Request::Tags`, the tags window opened on that series' first file).
- Only cards whose rect meets the visible area are laid out (`ScrollArea::show_viewport` with fixed card heights).

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn scans_come_before_other_files_which_start_closed() { /* with_odd_series(): order of card ids; Other closed */ }
#[test] fn a_card_shows_the_plain_name_and_the_description_beneath() {}
#[test] fn a_faulted_series_has_a_mark_and_no_warning_words_in_the_list() { /* no "several planes" text in the rail */ }
#[test] fn opening_a_faulted_series_posts_its_sentence() {}
#[test] fn the_patients_name_is_nowhere_in_the_rail_while_details_are_off() { /* every galley text in the pass */ }
#[test] fn pages_show_an_icon_not_a_thumbnail_while_details_are_off() {}
#[test] fn two_people_are_said_and_grouped() { /* made-up export of two patient ids */ }
#[test] fn an_export_of_no_scans_opens_other_files_and_shows_the_first() { /* Review Focus 2 */ }
#[test] fn a_long_name_is_cut_and_never_widens_the_rail() { /* Review Focus 3: 300-char description */ }
#[test] fn only_cards_in_sight_are_laid_out() { /* 500 made-up series: cards laid out <= visible + 2 */ }
#[test] fn a_cards_menu_shows_it_or_its_tags() { /* right-click a card: two items; each sends its action or request */ }
```

- [ ] **Step 2: Run** `cargo test -p voxa-app rail` — FAIL. **Step 3: Implement.** **Step 4: Run** `cargo test -p voxa-app` — PASS.
- [ ] **Step 5: Capture** the phantom at 1400×900: "a picture in colour" sits under Other files; the scans show plain names. Fill "Shown by" for UI-09, UI-10, UI-12, UI-13, UI-14.
- [ ] **Step 6: Commit** `feat(app): a rail of scans with plain names; paperwork folded into Other files`

## Phase 3: profiles and measuring

### Task 12: Profiles in the View tab, and the value under the pointer

Spec 7.6. Requirements UI-17 to UI-20. Risk R-UI-02.

**Files:**
- Modify: `crates/voxa-app/src/ui/tabs/view.rs`, `crates/voxa-app/src/ui/viewport.rs` and `viewport/annotate.rs` (the readout line), `crates/voxa-app/src/ui/planes/overlay.rs` (the readout over a plane), `crates/voxa-app/src/ui/commands/table/window.rs` (one command per CT window, `profiles: &[Profile::Ct]`)
- Test: `crates/voxa-app/src/ui/tabs/view/tests.rs`, `crates/voxa-app/src/ui/viewport/tests.rs`

**Interfaces:**
- Consumes: `profile::{Profile, CT_WINDOWS, unit_of, readout}`; `voxa_volume::sample(volume, point)` for planes; the shown `Frame` for slices.
- Produces: View tab brightness by profile: CT → the four named windows then Auto, each hint `Width {w}, level {l} HU`; MR → Auto, Soft, Hard (today's `PRESETS`); Basic → Auto. Colour pictures → `In its own colours: no brightness to set.` Fine-tune fields say `HU` when `unit_of` is Hu. While the pointer is over a picture, the bottom-left corner's last line is `Value {readout}`.

- [ ] **Step 1: Write the failing tests:** `a_ct_scan_offers_brain_soft_tissue_lung_bone_and_auto`; `pressing_brain_sets_width_80_level_40_on_the_active_viewport` (Action::SetWindow); `an_mr_scan_offers_auto_soft_hard`; `a_basic_scan_offers_auto`; `the_value_under_the_pointer_is_in_hu_for_ct` (made-up CT frame value −12 at the pointer → corner text contains `Value -12 HU`); `a_non_hu_ct_value_has_no_unit` (rescale type `US`).
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement.** **Step 4: Run** — PASS. Fill "Shown by" UI-17 to UI-20.
- [ ] **Step 5: Commit** `feat(app): the View tab follows the scan's profile; CT windows and HU under the pointer`

### Task 13: Measuring on the slices

Spec 7.7, section 5 (file version). Requirements UI-22, UI-23, UI-24. Review Focus 4. Risk R-UI-03, R-UI-06. Usability U-09.

**Files:**
- Create: `crates/voxa-app/src/measuring.rs`, `crates/voxa-app/src/measuring/tests.rs`, `crates/voxa-app/src/ui/viewport/tools.rs` (mouse to drafts on a slice)
- Modify: `crates/voxa-app/src/annotations.rs` (`Annotation::scale`), `crates/voxa-app/src/annotations/kept.rs` (`ANNOTATIONS_VERSION = 2`, reads 1), `crates/voxa-app/src/reducer/annotations.rs` (what is measurable), `crates/voxa-app/src/measurements.rs` (CSV/JSON column), `crates/voxa-app/src/ui/viewport.rs` (draw annotations on a slice, using `planes::shapes`)

**Interfaces:**
- Consumes: Task 1's `spacing_given`; `planes::tools::{drafted, preview, Draft, Pointer}` unchanged; `planes::shapes::{shown_on, drawn, paint}`.
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Scale { #[default] Patient, PixelSpacing }
pub struct SlicePlacement { pub placement: Affine<Voxel, Patient>, pub scale: Scale }
pub fn placement_of(info: &SeriesInfo) -> Option<SlicePlacement>;
    // voxel_to_patient Some and spacing_given -> Patient; no geometry, spacing_given -> an affine
    // of the pixel spacing alone (x = col·sx, y = row·sy, z = slice) with PixelSpacing; else None
pub fn slice_plane(at: &SlicePlacement, slice: usize, size: [u32; 2]) -> Plane;
pub fn frame_ellipse_stats(frame: &Frame, at: &SlicePlacement, slice: usize, centre: [f64; 3],
                           radii_mm: [f64; 2], across: [f64; 3], down: [f64; 3]) -> Option<RegionStats>;
pub fn why_not_measurable(state: &AppState) -> Option<&'static str>;
    // None when grid(state) is Some or the active viewport's series has a placement and a frame shown;
    // else "Measuring needs a scan with a scale in its files."
```

  `Annotation` gains `#[serde(default)] pub scale: Scale`. A measurement with `Scale::PixelSpacing` shows ` (from the file's pixel spacing)` after its value in the list and on the picture, and the CSV/JSON gain a column `scale` with `patient` or `pixel spacing`.
- Reducer: `SetTool` and `AnnotationAdd` are allowed when `why_not_measurable(state).is_none()` (today: only with a volume). `annotations::settle` puts the tool down on the same rule. A half-made draft lives in the UI (`viewport::tools::Drafts`, one per viewport) and is dropped whenever that viewport's series or slice changes.
- On a slice, annotations of the scene's series are drawn when `shown_on(shape, slice_plane(..), half the slice thickness)`.

- [ ] **Step 1: Write the failing tests** in `measuring/tests.rs` and `reducer/annotations_tests.rs`:

```rust
#[test] fn a_scan_with_geometry_is_measured_in_the_patients_millimetres() { /* thick_scan_ready series: Scale::Patient */ }
#[test] fn a_scan_with_spacing_only_is_measured_within_the_picture() { /* spacing_given, no geometry: Scale::PixelSpacing; a ruler across 10 columns at 0.5 mm = 5.0 mm */ }
#[test] fn a_scan_without_spacing_is_not_measured_and_says_why() { /* why_not_measurable == Some("Measuring needs a scan with a scale in its files.") */ }
#[test] fn a_ruler_can_be_drawn_on_a_slice_before_the_volume_is_ready() { /* scene status Building: SetTool(Ruler) then AnnotationAdd accepted */ }
#[test] fn a_region_on_a_slice_takes_its_values_from_that_slice() { /* made-up frame of 7s inside: mean 7, sd 0 */ }
#[test] fn measurements_kept_by_0_6_0_still_open() { /* a version-1 file text with one ruler: loads with Scale::Patient */ }
#[test] fn a_half_drawn_angle_is_dropped_when_the_scan_changes() { /* Review Focus 4: two presses, SelectSeries other, third press: no AnnotationAdd for either series */ }
#[test] fn measurements_from_pixel_spacing_say_so_in_the_list_and_the_csv() {}
```

- [ ] **Step 2: Run** `cargo test -p voxa-app measuring annotations` — FAIL.
- [ ] **Step 3: Implement.** Reuse `drafted`/`preview` with the slice's `Plane`; turn pointer positions into patient points through the viewport's existing screen-to-pixel mapping and `SlicePlacement::placement`.
- [ ] **Step 4: Run** `cargo test -p voxa-app` — PASS. Capture with `VOXA_CAPTURE_SHOW=measured` on Slices.
- [ ] **Step 5: Commit** `feat(app): measure on the slices as soon as they show, and say where the millimetres come from`

### Task 14: Measurement labels never overlap

Spec 7.7. Requirement UI-25. Risk R-UI-04. Usability U-07.

**Files:**
- Create: `crates/voxa-app/src/ui/labels_placed.rs` (pure placement), tests
- Modify: `crates/voxa-app/src/ui/planes/shapes.rs` (`paint` takes a placed rect), `crates/voxa-app/src/ui/scene/tools.rs` (3D labels), `crates/voxa-app/src/ui/viewport.rs` (slices)

**Interfaces:**
- Produces: `pub fn place(wanted: &[Rect], bounds: Rect) -> Vec<Placed>` with `Placed { rect: Rect, leader: Option<(Pos2, Pos2)> }`: labels in order; each keeps its wanted rect if it meets no earlier placed rect, else moves down (then up) in steps of its own height within `bounds` until clear, with a leader line from the shape's anchor to the moved label.

- [ ] **Step 1: Write the failing tests:** `labels_that_do_not_meet_stay_put`; `crowded_labels_are_moved_apart_with_leaders` (four rects at one point: no two placed rects intersect); `a_label_stays_inside_its_picture`.
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement.** **Step 4: Run** — PASS; capture `VOXA_CAPTURE_SHOW=measured` with Scene: no overlapping labels (compare with today's `now-parts.png`).
- [ ] **Step 5: Commit** `fix(app): measurement labels move aside rather than overlap`

## Phase 4: documents

### Task 15: Reports and PDFs in the index (voxa-dicom)

Spec 7.15. Requirement UI-34, UI-35 (data). Risk R-UI-10. Security note S-UI-01.

**Files:**
- Create: `crates/voxa-dicom/src/document.rs`, `crates/voxa-dicom/src/document/report.rs`, `crates/voxa-dicom/src/document/tests.rs`
- Modify: `crates/voxa-dicom/src/index.rs:73-78` (a non-image DICOM file becomes a document, not a skip), `crates/voxa-dicom/src/lib.rs`, `crates/voxa-dicom/src/testkit.rs` (make a made-up SR and an encapsulated PDF)

**Interfaces:**
- Produces:

```rust
pub enum DocumentKind { Report, Pdf, Other(String) }   // Other: the SOP class in words where known
pub struct IndexedDocument { pub entry: String, pub study_uid: String, pub kind: DocumentKind,
                             pub title: String, pub date: Option<String> }
// Index gains: pub documents: Vec<IndexedDocument>   (in study order, then entry name)
pub struct ReportLine { pub depth: u8, pub heading: Option<String>, pub text: String }
pub struct Report { pub lines: Vec<ReportLine>, pub cut_short: bool }
pub fn read_report(bytes: &[u8]) -> Result<Report, VoxaError>;
pub fn pdf_bytes(bytes: &[u8]) -> Result<Vec<u8>, VoxaError>;   // (0042,0011), trailing pad removed
pub const MAX_REPORT_DEPTH: usize = 16;
pub const MAX_REPORT_LINES: usize = 5_000;
pub const MAX_REPORT_CHARS: usize = 1_000_000;
```

  A file is a document when it parses as DICOM with a SOP Class UID and a Study Instance UID but has no image. Report: SR storage classes (1.2.840.10008.5.1.4.1.1.88.*); Pdf: 1.2.840.10008.5.1.4.1.1.104.1. `read_report` walks the Content Sequence (0040,A730) in order: CONTAINER → heading from the Concept Name's Code Meaning; TEXT/CODE/NUM (with units' Code Meaning)/DATE/TIME/DATETIME/PNAME/UIDREF → a line `{concept}: {value}`; deeper than `MAX_REPORT_DEPTH`, more lines or characters than the limits → stop and set `cut_short`. Never panics; a damaged report is an `Err`.

- [ ] **Step 1: Write the failing tests:** `a_report_is_listed_as_a_document_not_skipped`; `a_pdf_is_listed_as_a_document`; `a_report_reads_as_lines_in_its_order` (made-up SR: heading "Findings"-free made-up concept names such as "Comment"; a TEXT and a NUM with "mm"); `a_report_nested_too_deep_is_cut_short_and_says_so`; `a_damaged_report_is_an_error_not_a_panic` (truncated bytes); `the_pdf_inside_is_given_back_whole`; `images_are_still_indexed_as_before` (existing tests unchanged).
- [ ] **Step 2: Run** `cargo test -p voxa-dicom document` — FAIL. **Step 3: Implement.** **Step 4: Run** `cargo test -p voxa-dicom` — PASS.
- [ ] **Step 5: Write security note** S-UI-01 in `SECURITY.md` under a heading "Reading reports": limits, no rendering of PDFs, nothing executed.
- [ ] **Step 6: Commit** `feat(dicom): reports and PDFs in an export are listed as documents, and a report reads as text`

### Task 16: Documents in the window

Spec 7.15. Requirements UI-34, UI-35. Risk R-UI-10.

**Files:**
- Create: `crates/voxa-app/src/ui/document.rs`, tests
- Modify: `crates/voxa-app/src/state.rs` (`document: Option<OpenDocument>`), `crates/voxa-app/src/action.rs` (`OpenDocument(String)`, `CloseDocument`, `DocumentRead { entry: String, report: Result<Report, String> }`), `crates/voxa-app/src/effect.rs` (`ReadDocument { entry }`, `SaveDocumentPdf { entry, to: PathBuf }`), `crates/voxa-app/src/worker.rs` (both effects; reading through the open source's `read(name)`), `crates/voxa-app/src/reducer.rs`, `crates/voxa-app/src/ui/rail.rs` (documents under Other files), `crates/voxa-app/src/ui/pictures.rs` (an open document takes the pictures' area)

**Interfaces:**
- Consumes: Task 15's types.
- Produces: `pub struct OpenDocument { pub entry: String, pub report: Option<Result<Report, String>> }`. `SelectSeries` closes an open document. The document view: for a Report, the heading line `Shown as text. The original report is the reference.`, then the lines indented by depth, selectable text, and when `cut_short` the line `Part of this report is too long or too deeply nested to show here.`; for a Pdf, `This report is a PDF. Voxa does not show PDFs.` and the button `Save a copy as PDF…` (a system save dialog; the file is written only where chosen); for Other, its kind in words and `Voxa cannot show this kind of file.`; each with the file's tags (today's tags panel, opened on that entry).

- [ ] **Step 1: Write the failing tests:** `opening_a_report_reads_it_on_a_worker_and_shows_its_lines`; `the_report_is_headed_as_not_the_reference`; `a_cut_short_report_says_so`; `a_pdf_offers_a_copy_and_starts_no_program` (the only effect of the button is `SaveDocumentPdf` with the chosen path); `choosing_a_scan_closes_the_document`.
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement.** **Step 4: Run** `cargo test -p voxa-app` — PASS.
- [ ] **Step 5: Commit** `feat(app): reports open as text and PDFs can be saved, from Other files`

## Phase 5: look, access and help

### Task 17: Light and dark themes

Spec 7.13. Requirement UI-29. Risk R-UI-09.

**Files:**
- Modify: `crates/voxa-app/src/ui/theme.rs` (split into `theme.rs` and `theme/palette.rs`), every file using `theme::QUIET_TEXT`, `WARNING`, `PANEL`, `RAISED`, `ACCENT` (about 55 uses), `crates/voxa-app/src/prefs.rs` (`theme: String`), `crates/voxa-app/src/ui/top.rs` (the theme button: System → Light → Dark)

**Interfaces:**
- Produces: `pub enum ThemeChoice { #[default] System, Light, Dark }`; `pub struct Palette { pub panel, pub raised, pub text, pub quiet_text, pub accent, pub warning: Color32 }`; `pub fn palette(ctx: &egui::Context) -> Palette` (by the context's current dark mode); `pub fn apply(ctx: &egui::Context, choice: ThemeChoice)`; `pub fn contrast(a: Color32, b: Color32) -> f32` (WCAG relative-luminance ratio). The pictures' colours (`BACKGROUND`, `AXIAL`, `SAGITTAL`, `CORONAL`, overlay text) stay constants, the same in both themes.

- [ ] **Step 1: Write the failing tests:** `each_theme_meets_aa_contrast` (for both palettes: text, quiet_text, accent, warning against panel and raised ≥ 4.5); `the_pictures_stay_dark_in_the_light_theme` (BACKGROUND unchanged; a headless pass in Light draws the viewport fill as BACKGROUND); `the_theme_choice_is_remembered`; update `the_theme_is_dark_with_the_accent_on_selections_and_rounded_corners` to the Dark choice.
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement**, replacing each constant use with `theme::palette(ui.ctx()).field`. **Step 4: Run** `cargo test -p voxa-app` — PASS. Capture both themes at 1400×900 (`VOXA_CAPTURE_SHOW=light`).
- [ ] **Step 5: Commit** `feat(app): light and dark themes, the pictures dark in both`

### Task 18: Screen-reader names and keyboard order

Spec 7.14. Requirements UI-33, UI-04 (names from the table).

**Files:**
- Modify: `crates/voxa-app/src/ui/rail/card.rs`, `crates/voxa-app/src/ui/viewport.rs`, `crates/voxa-app/src/ui/planes.rs`, `crates/voxa-app/src/ui/scene.rs`, `crates/voxa-app/src/ui/strip.rs`, `crates/voxa-app/src/ui/tabs/labels/overview.rs`, `crates/voxa-app/src/ui/panel.rs`
- Create: `crates/voxa-app/src/ui/access_tests.rs`

**Interfaces:**
- Produces: every hand-drawn interactive response carries `widget_info` with a name: a card `{plain name}, {caption}{, selected}{, has a problem}`; a viewport `{Axial|Coronal|Sagittal|Slice} {n} of {m}, {plain name}`; the 3D scene `3D view of {plain name}`; strip buttons and folded-tab icons their command names. Arrow keys Up/Down move between cards when the rail has focus; Enter opens.

- [ ] **Step 1: Write the failing tests** (`ctx.enable_accesskit()`, read `platform_output.accesskit_update` through `egui::accesskit`; check the 0.36 API in the registry first): `every_interactive_node_has_a_name` (whole window with `thick_scan_ready()` and labels: every node whose role is Button, CheckBox, Tab, ListItem or Image-with-click has a non-empty label); `every_button_in_the_bars_and_tabs_is_a_command` (each Button node's label in the top bar, strip and tabs is a `Command::name`, or one of the hand-drawn names above); `a_card_says_its_name_and_whether_it_is_selected`; `a_viewport_says_which_slice_it_shows`; `the_rail_is_walked_with_the_arrow_keys`.
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement.** **Step 4: Run** — PASS. Fill "Shown by" UI-33.
- [ ] **Step 5: Commit** `feat(app): every control and picture has a name a screen reader can say`

### Task 19: Help topics, and the wording check

Spec 7.18, section 4 (anti-goals). Requirements UI-38, UI-39. Risk R-UI-08.

**Files:**
- Create: `crates/voxa-app/src/ui/help/topics.rs`, `crates/voxa-app/src/ui/help/wording_tests.rs`
- Modify: `crates/voxa-app/src/ui/help.rs` (topics above the keys; `Request::Help(Some(id))` opens at that topic), `crates/voxa-app/src/ui/search.rs` (Help heading from `TOPICS`)

**Interfaces:**
- Produces: `pub struct Topic { pub id: &'static str, pub title: &'static str, pub words: &'static [&'static str], pub body: &'static str }`; `pub const TOPICS: [Topic; 7]` with ids `open`, `find`, `brightness`, `measure`, `sides`, `labels`, `not-for`. Bodies are a few plain sentences each; `measure` says where millimetres come from (geometry, pixel spacing only, none) in the words of Task 13; `not-for` uses the intended-use statement's sense.
- Wording rule: no command name, hint, why-not text or topic body contains, as a whole word and ignoring case, `normal`, `abnormal`, `lesion`, `tumour`, `tumor`, `finding`, `findings`, `score`, `diagnosis`; `diagnostic` only within the disclaimer's sentence or `not for diagnostic use`.

- [ ] **Step 1: Write the failing tests:** `there_are_seven_topics_each_found_by_search`; `no_text_suggests_a_finding` (the wording rule over `commands::all()`, `TOPICS`, and the tab texts' consts); `the_measure_topic_says_where_millimetres_come_from`.
- [ ] **Step 2: Run** — FAIL. **Step 3: Write** the topics. **Step 4: Run** — PASS. Re-read each topic against `docs/quality/risk-register.md`.
- [ ] **Step 5: Commit** `docs(app): help topics found by search, and a check that no wording suggests a finding`

## Phase 6: fit and finish

### Task 20: Small windows, captures, the frame measurement and the documents

Spec 7.14, 7.16, 7.17, 8. Requirements UI-30, UI-31, UI-32, UI-36, UI-37.

**Files:**
- Modify: `crates/voxa-app/src/ui/screen.rs` (below the NARROW width the rail starts folded; below 760 points both fold), `crates/voxa-app/src/ui/capture.rs` (words `tab-view`, `tab-measure`, `tab-3d`, `tab-labels`, `tab-tools`, `search`, `light`, `other`, `notices`; documented in its module comment), `crates/voxa-app/examples/` (new `measure_window.rs`: lays out the window headless with a made-up study of 500 series for 200 frames and prints the median frame time; run on 0.6.0's tag of the same example logic for the comparison), `docs/intended-use.md` (MRI and CT scans, and other DICOM files at a basic level), `CHANGELOG.md` ("Unreleased"), `docs/architecture.md` (the window's parts and the list of actions), `docs/quality/requirements.md` ("Shown by" complete)
- Test: `crates/voxa-app/src/ui/screen/tests.rs`, `crates/voxa-app/src/prefs/tests.rs`

- [ ] **Step 1: Write the failing tests:** `at_640_by_480_both_columns_fold_and_the_top_bar_keeps_open_search_and_the_menu`; `a_0_6_0_settings_file_opens_with_its_settings` (a literal 0.6.0 prefs JSON: window, interface size, list share, viewports, radiological, linked, overlay all honoured; new fields default); `widths_and_folds_are_remembered`.
- [ ] **Step 2: Run** — FAIL. **Step 3: Implement.** **Step 4: Run** `cargo test -p voxa-app` — PASS.
- [ ] **Step 5: Measure** with `cargo run --release -p voxa-app --example measure_window`, on this branch and on `release`, with nothing else heavy running; record both medians in `docs/quality/usability.md` §6 as a note and in UI-37's "Shown by".
- [ ] **Step 6: Commit** `feat(app): small windows fold the columns; captures, frame measurement and documents brought up to date`

### Task 21: Acceptance

Spec 3, 8. All requirements.

**Files:**
- Create: `docs/acceptance-ui-redesign.md`
- Modify: `docs/quality/usability.md` (§6 a dry run), `docs/quality/risk-register.md` ("Checked by" filled with test names)

- [ ] **Step 1:** `cargo xtask check` — green; note the test count.
- [ ] **Step 2:** Captures with `write_phantom` (and `write_phantom head …` for labels) at 640×480, 800×600, 1400×900, in both themes, with each tab, search open, Other files open, a report open: kept in the scratchpad, not the repository; describe what each shows in the acceptance record.
- [ ] **Step 3:** Dry run of the success test on the phantom by the builder, following the usability script, recorded as a dry run (not a participant).
- [ ] **Step 4:** Check every requirement row has "Shown by" filled and every risk row "Checked by"; list any that are not, with why.
- [ ] **Step 5: Commit** `docs: acceptance record of the redesigned window`
