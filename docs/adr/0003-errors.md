# 3. Errors are values; nothing a file contains may crash Voxa

> Volsa was earlier called Voxa. This record keeps the name it was written with.

Date: 2026-10-05. Status: accepted.

## Context

Scan exports are messy: reports and viewers mixed in with images, truncated files, private
transfer syntaxes, series that are not volumes. A viewer that crashes on one bad file loses the
user's trust in everything else it shows.

## Decision

- Library crates never panic. `unwrap`, `expect`, `panic!` and indexing that can panic are denied
  by lint outside tests. Failures are returned as `thiserror` enums.
- A file that cannot be read is skipped and counted; the interface says how many and which.
- A series that cannot be shown as intended (mixed sizes, missing geometry, unsupported
  compression) is still listed, with the reason in plain words.
- One slice that fails to decode shows a placeholder for that slice only.
- Error messages shown to the user say what happened and what to do, without internal names.
- Logs never contain patient-identifying values.

## Consequences

- More `Result` plumbing and explicit handling at each layer.
- Indexing is fuzzed and property-tested: random bytes must produce `Ok` or an error, never a
  panic.
- Problems are data (`SeriesProblem`), so the interface can explain them and tests can assert on
  them.
