# Current Status

Baseline date: 2026-09-03.

This file records the restart baseline and should be updated when the architectural phase changes materially. Detailed historical parity/tranche documents remain useful context but may overstate runtime completeness.

## Native Windows restart baseline

Observed on Windows 11 with the stable MSVC Rust toolchain:

- the workspace compiled natively on Windows;
- `caliberate-gui.exe` built and launched;
- runtime directories under `.cache/caliberate` initialized successfully;
- the initial surviving CLI test log showed one failure and 51 passes;
- the GUI build emitted a substantial warning backlog (57 warnings in the observed run), including deprecated egui calls, dead fields, and unused rendering helpers.

### Windows path regression — resolved

Task `0001-windows-path-identity` fixed the failing `ebook_convert_rejects_input_equals_output` case by comparing canonical paths when the output already exists. Luna/Codex ran the focused test plus the full workspace suite in its native Windows environment; all passed.

Accepted commit: `3bbc5f10a45ec68ab9f4ff8f556432c44cae1268`.

`main` has been fast-forwarded to include the fix and task report.

## Current product priority

The near-term product is now explicitly the **visual library platform**, not a full Calibre feature port in arbitrary order.

P0:

- Calibre-like visual desktop browsing/searching;
- reusable library/query/content service for GUI and sibling projects;
- managed, arbitrary-directory, and attached-Calibre library sources;
- HTTP/JSON and OPDS adapters over the same service semantics;
- large-library search/facet/sort/pagination behavior.

P1:

- real reader formats;
- TTS;
- conversion depth;
- metadata depth;
- broader CLI automation.

P2/deferred unless needed by higher-priority work:

- device integration expansion;
- ebook editor/polishing;
- news acquisition;
- email delivery;
- plugin ecosystem expansion.

See `docs/project/priorities.md` and `docs/roadmaps/roadmap-visual-library-platform.md`.

## Library reality

The existing library/asset code already has meaningful ingest and copy/reference behavior, but the reusable service/source model is not yet established end-to-end.

Not yet first-class:

- arbitrary directory-backed libraries with persistent rescan/reconciliation while leaving files in place;
- flat-directory source workflow;
- attached existing Calibre library with Calibre absent;
- clean separation between externally owned source data and Caliberate overlay state;
- a shared library/query/content facade consumed by GUI/server/other projects.

The current OPDS implementation still opens `caliberate_db::Database` directly inside protocol handlers. This is P0 architectural debt: protocol adapters should consume the shared library service instead.

## Visual GUI reality

The GUI already contains substantial library shell/state work, but source is highly concentrated and should not be treated as the target architecture.

At restart:

- `crates/gui/src/views.rs` is roughly 493 KB;
- `crates/gui/src/app.rs` is roughly 65 KB;
- `crates/gui/src/preferences.rs` is roughly 68 KB.

The P0 visual target is recognizably Calibre-like information architecture: action toolbar, global/advanced search, left category/tag browser, central list/cover-grid browsing, optional cover browser, right book-details panel, virtual libraries, and persistent layout controls.

## Reader reality

The current GUI contains a large amount of reader shell/state behavior, but `ReaderContent::from_path` meaningfully loads only:

- `txt`
- `md`
- `markdown`

EPUB, PDF, DOCX, MOBI/AZW, and HTML are not real GUI reader loaders yet.

Reader expansion remains P1 behind the visual library/service platform.

## TTS reality

No reader TTS implementation was found at restart. The future speech abstraction can therefore be introduced cleanly, but it is not the immediate P0 focus.

## Conversion reality

The conversion CLI and orchestration exist, but practical cross-format conversion remains largely unimplemented beyond passthrough behavior. Finished core conversion must not require a Calibre installation.

## Immediate work queue

1. `0002-cross-platform-ci` — add Windows + Linux CI baseline.
2. Define and introduce the first read-only library/query/content service seam.
3. Inventory/refactor direct DB usage from OPDS/GUI into that service incrementally.
4. Build the Calibre-like visual library shell against the service.
5. Add HTTP/JSON API over the same semantics and refactor OPDS to the same service.
6. Deepen library-source support and search/facet/virtual-library behavior.

## Completion standard

A roadmap checkbox, UI control, struct field, or stub does not establish feature completion. Completion requires executable behavior plus evidence appropriate to the feature: tests, runtime validation, or both.
