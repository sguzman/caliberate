# GUI Parity Validation Snapshot (2026-04-13)

## Method
- Checked roadmap checkbox completion in `docs/roadmaps/gui/*.md`.
- Cross-checked implementation behavior in Rust GUI sources for parity-critical flows.
- Logged concrete parity gaps as new unchecked items in roadmap files.

## Checkbox Completion
- `collections.md`: 13/13 (100.0%)
- `conversion.md`: 17/17 (100.0%)
- `device.md`: 13/13 (100.0%)
- `library.md`: 56/56 (100.0%)
- `metadata.md`: 74/74 (100.0%)
- `news.md`: 12/12 (100.0%) before this audit
- `plugins.md`: 9/9 (100.0%) before this audit
- `reader.md`: 21/21 (100.0%) before this audit
- `settings.md`: 40/40 (100.0%) before this audit
- `shell.md`: 78/78 (100.0%) before this audit

## Parity Findings
- Checkbox completion did not imply full Calibre GUI parity.
- Concrete implementation gaps found:
  - News downloads still generate placeholder digest files (`crates/gui/src/views.rs:7664`).
  - Auto cover generation is a placeholder renderer (`crates/gui/src/views.rs:6825`).
  - Plugin manager is seeded from in-memory defaults, not filesystem/plugin packages (`crates/gui/src/views.rs:10741`).
  - Reader TOC is reconstructed from heading scans instead of source-native TOC structures (`crates/gui/src/views.rs:9815`).

## Roadmap Updates Applied From This Validation
- Added parity-gap backlog items (unchecked) to:
  - `docs/roadmaps/gui/news.md` (6 items)
  - `docs/roadmaps/gui/plugins.md` (8 items)
  - `docs/roadmaps/gui/reader.md` (8 items)
  - `docs/roadmaps/gui/shell.md` (4 items)
  - `docs/roadmaps/gui/settings.md` (4 items)
- Total new parity-gap items added: 30.

## Result
- GUI roadmap folder now tracks known parity gaps explicitly instead of showing false 100% completion.
