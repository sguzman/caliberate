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

## Cross-platform CI — integrated

Task `0002-cross-platform-ci` added `.github/workflows/cross-platform-ci.yml` with a Windows/Linux GitHub Actions matrix. Each job runs formatting, locked workspace checking, and locked workspace tests.

Luna/Codex validated the commands natively on Windows before handoff. The workflow itself provides hosted Windows/Linux evidence on subsequent GitHub runs.

Accepted commit: `bb21ab25babfe01e7094ea49c13918ee5c896347`.

## Library catalog facade — integrated

Task `0003-library-catalog-facade` introduced the first read-only library-domain seam:

- `caliberate_library::catalog::LibraryBook`
- `caliberate_library::catalog::LibraryCatalog`

The facade delegates list/get/search operations to the existing database and maps database records into library-domain DTOs. No SQL or database behavior was duplicated in the library crate.

Accepted commit: `2d8a5dc7a213c946389913477794d7af67456d14`.

## OPDS catalog facade adoption — integrated

Task `0004-opds-use-library-catalog` routed OPDS list, entry, and search reads through `LibraryCatalog` while preserving existing protocol behavior.

Accepted commit: `61ae4917855bb64f6d1aee41c2abfca226542a0e`.

## Library content locator — integrated

Task `0005-library-content-locator` added:

- `caliberate_library::catalog::LibraryContent`
- `LibraryCatalog::resolve_content(book_id)`

The locator preserves the existing storage-selection rule: prefer the first `copy` asset, otherwise the first asset in database order, otherwise the logical book path. Missing logical books return `None`. Server authorization, external-path policy, MIME handling, size limits, and filesystem checks remain outside the library crate.

Accepted commit: `7748255c04d812208221ff62704513694431b098`.

## Current product priority

The near-term product is explicitly the **visual library platform**, not a full Calibre feature port in arbitrary order.

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

The first reusable read-only library-domain facade now exists for basic catalog operations and content resolution.

OPDS list/get/search reads consume the facade rather than direct database catalog methods. The remaining OPDS download path still opens the database and duplicates asset selection; task `0006-opds-download-use-content-locator` is queued to remove that duplication while retaining server-side authorization and filesystem policy.

Still not first-class:

- structured library-domain query/facet/sort/pagination semantics;
- arbitrary directory-backed libraries with persistent rescan/reconciliation while leaving files in place;
- flat-directory source workflow;
- attached existing Calibre library with Calibre absent;
- clean separation between externally owned source data and Caliberate overlay state;
- common facade adoption by GUI and future HTTP/JSON consumers.

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

1. `0006-opds-download-use-content-locator` — route OPDS download content selection through `LibraryCatalog::resolve_content` while keeping HTTP/server policy in the server.
2. Introduce structured library-domain query/facet/sort/pagination semantics needed by visual browsing and APIs.
3. Add HTTP/JSON adapter over the same library service.
4. Move the Calibre-like GUI browsing/search path onto the common service and decompose the relevant GUI seams as needed.
5. Deepen library-source support: directory-backed and attached-Calibre modes.

## Completion standard

A roadmap checkbox, UI control, struct field, or stub does not establish feature completion. Completion requires executable behavior plus evidence appropriate to the feature: tests, runtime validation, or both.
