# Risk register

What could lead someone to be misled or harmed by using Volsa, what keeps that from
happening, and how that is checked. Kept in the spirit of ISO 14971. Volsa is not a medical
device and is not for diagnostic use; the register exists because people will still look
at their own scans with it and draw conclusions.

How bad (severity): **minor** (confusion, wasted time), **serious** (a person believes
something false about their own or someone's body, and may worry, delay or act on it),
**critical** (care is changed because of Volsa). How likely, before the control:
**rare**, **possible**, **likely**. Each entry is reviewed whenever the part of Volsa it names
changes.

Entries from earlier work (labels on the wrong grid, left and right, a failed model check
hidden) are brought in when that work is next changed.

## PL: plugins (design: 2026-10-31-volsa-plugins-design.md)

The tests named are listed in full against their requirements in [requirements.md](requirements.md),
section PL.

| No. | Hazard and how harm could follow | Severity / likelihood before | Control | Checked by | After |
|---|---|---|---|---|---|
| R-PL-01 | A plugin's labels are taken for Volsa's own, or for checked ones, and believed. | serious / likely | The banner first wherever they are shown or written (PL-05, PL-11); the plugin's words only inside the frame (PL-20); no diagnostic wording in Volsa's own text. | PL-05, PL-11 and PL-20's tests; the card and the Parts tab looked at in pictures at 900×600 and 640×480. The plane and 3D captions carry the banner's short form, not the stamps (PL-11, partly met). | serious / rare |
| R-PL-02 | A plugin is run on a scan it was not made for (another modality or contrast), and gives confident wrong labels. | serious / likely | Modality refused (PL-08); contrast warned and stamped (PL-09, PL-10); the plugin's accepts and limits on its entry and card. | PL-08, PL-09 and PL-10's tests; `volsa-app/src/ui/plugins/tests.rs::the_warning_runs_only_on_run_anyway`. | serious / possible |
| R-PL-03 | A description states the preparation wrongly (scaling, axes, spacing), and the labels look plausible but are wrong. | serious / possible | No defaults for answer-changing settings (PL-04); every setting in plain words on the card (PL-05); each setting tested to do what it says (PL-13, PL-14). Residual: Volsa cannot know a setting is wrong; the card says Volsa has not checked the model. | PL-04, PL-05, PL-13 and PL-14's tests. | serious / possible |
| R-PL-04 | Labels are mapped back to the wrong place or the wrong side. | serious / possible | One generalised mapping, tested with an off-centre bead for every axis code (PL-12); SynthSeg's mapping unchanged (PL-16). | `volsa-plugins/src/grid/bead_tests.rs` (all 48 axis codes, a scan stored the other way round, a tilted scan); PL-16. | serious / rare |
| R-PL-05 | A hostile network file runs code, or crashes Volsa and loses unsaved work. | serious / possible | Standard operators only and the reader's limits (PL-03); the helper process, ended with Volsa, starting nothing (PL-07). Residual: a fault in ONNX Runtime itself runs with the person's own rights; SECURITY.md says to add plugins only from sources they trust. | PL-03's tests, among them `volsa-plugins/src/onnx/limit_tests.rs::random_bytes_never_panic`; PL-07's tests; `volsa-models/tests/helper.rs`. | serious / rare |
| R-PL-06 | A plugin's files are changed after it was added, and something else runs under its name. | serious / rare | Files copied into Volsa's folder; sums checked by the helper before every run (PL-02). | `volsa-plugins/src/helper/tests.rs::a_changed_network_file_is_refused_before_it_is_opened`; `volsa-plugins/src/run/tests.rs::a_plugin_whose_folder_name_is_not_its_sum_is_not_installed`. | serious / rare |
| R-PL-07 | A description's names lead a copy or a read outside the folder. | serious / possible | Plain names only, no links followed, a folder named by a sum (PL-01, PL-02). | `volsa-plugins/src/folder/tests.rs::a_name_with_a_separator_or_dots_is_refused`, `::a_link_is_refused`, `::two_names_differing_only_in_case_are_refused`; `volsa-plugins/src/onnx/tests.rs::external_data_named_with_dots_or_unlisted_or_past_the_end_is_refused`. | minor / rare |
| R-PL-08 | Volsa is taken to vouch for a plugin, or for its licence. | minor / possible | Never downloads or links (PL-19); the licence is the plugin's own words and the person's responsibility, said in docs/plugins.md and docs/intended-use.md. | `volsa-app/src/ui/plugins/tests.rs::a_source_is_text_not_a_link`. | minor / rare |
| R-PL-09 | A run takes so much memory that the computer slows or another program loses work. | serious / likely | Measured once (PL-06); a run starts only where it fits; the system caps the helper; blocks keep Volsa's own part small (PL-15); one run at a time (PL-23). | PL-06, PL-15 and PL-23's tests; `volsa-plugins/tests/with_runtime.rs::a_cap_below_the_networks_need_is_refused_not_obeyed` (the system refuses the helper memory past its cap). | minor / rare |
| R-PL-10 | A run stops part-way and a part-made result is shown as whole. | serious / rare | Only a finished answer becomes a result; a helper that ends early is a failure said in a sentence (PL-07). | `volsa-plugins/src/helper/tests.rs::a_helper_that_ends_mid_block_is_a_failure_not_a_hang`; `volsa-app/src/worker/plugins_run_tests.rs::a_helper_that_ends_early_says_so_and_volsa_goes_on`. | serious / rare |
| R-PL-11 | Labels near block edges differ from the model's own program's, and a small structure there is judged absent. | minor / possible | Said on the result when blocks were used. | `volsa-plugins/src/run/tests.rs::a_block_run_gives_the_same_labels_and_the_blocks_note`. | minor / possible |
| R-PL-12 | A result made by an older version, or by a removed plugin, is taken for the newer one's. | minor / possible | Version and sum on every result (PL-17, PL-18); a result is kept under the plugin's sum, so a new version does not open an old one's result. | `volsa-labels/src/made/added_tests.rs::added_by_you_round_trips_through_a_written_table`; `volsa-app/src/worker/plugins_run_tests.rs::a_run_is_kept_for_its_series_and_plugin_and_found_again`. | minor / rare |
| R-PL-13 | Pixels of a scan are left on disk by a plugin run. | serious / possible | Pipes only, nothing written (PL-21). | PL-21's tests, and reading `volsa-plugins/src/run.rs` and `helper/`. | serious / rare |
