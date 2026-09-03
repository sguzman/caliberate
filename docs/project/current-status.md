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

Accepted commit: `bb21ab25babfe01e7094ea49c13918ee5c896347`.

## Library catalog facade — integrated

Task `0003-library-catalog-facade` introduced the first read-only library-domain seam:

- `caliberate_library::catalog::LibraryBook`
- `caliberate_library::catalog::LibraryCatalog`

Accepted commit: `2d8a5dc7a213c946389913477794d7af67456d14`.

## OPDS catalog facade adoption — integrated

Task `0004-opds-use-library-catalog` routed OPDS list, entry, and search reads through `LibraryCatalog` while preserving existing protocol behavior.

Accepted commit: `61ae4917855bb64f6d1aee41c2abfca226542a0e`.

## Library content locator — integrated

Task `0005-library-content-locator` added `LibraryContent` and `LibraryCatalog::resolve_content(book_id)`. The locator preserves copy -> first asset -> logical book path selection while leaving HTTP/filesystem policy outside the library crate.

Accepted commit: `7748255c04d812208221ff62704513694431b098`.

## OPDS content-locator adoption — integrated

Task `0006-opds-download-use-content-locator` routed OPDS download storage selection through `LibraryCatalog::resolve_content`. Server authorization, filesystem checks, size limits, MIME mapping, response status, and streaming remain server-owned.

Accepted commit: `3f7c47a526d39c78ee7b7dab5e3c9fb80d70a928`.

## Library structured query and facets — integrated

Task `0007-library-query-facets` added library-domain structured filters and author/tag/series/publisher/rating/language facet values over the existing database behavior.

Accepted commit: `2f43fc15853820d23eb117783ec6837ec9261f84`.

## Library sorting, pagination, and totals — integrated

Task `0008-library-query-pages` extended structured queries with:

- deterministic ID/title/format sorting;
- ascending/descending direction;
- database-backed limit/offset pagination, including offset without limit;
- filtered `COUNT(DISTINCT b.id)` totals;
- `LibraryQueryPage` and `LibraryCatalog::query_page`.

The DB count and result queries share the same filter/join construction. Sorting is selected through enums mapped to hard-coded SQL expressions, and non-ID sorts use ID as a deterministic tie-breaker.

Accepted commit: `d3f4d21399064d811236324fb724aa6edc236163`.

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

The reusable read-only library-domain facade now exists for basic catalog operations, content resolution, structured filtering, category facets, deterministic sorting, database-backed pagination, and filtered totals.

OPDS list/get/search/download storage selection consume that library facade. Protocol-specific authorization and wire behavior remain in the server.

The next missing seam is an efficient rich book-summary page matching the metadata already displayed by the GUI. Task `0009-library-book-summaries` is queued to provide structured authors/tags/series/publisher/rating/languages/cover/date metadata with fixed-count page-wide batch queries rather than N+1 per-book lookups.

Still not first-class:

- the rich `LibraryBookSummary` read model (`0009` queued);
- broader sort fields such as author/series/rating/date;
- arbitrary directory-backed libraries with persistent rescan/reconciliation while leaving files in place;
- flat-directory source workflow;
- attached existing Calibre library with Calibre absent;
- clean separation between externally owned source data and Caliberate overlay state;
- common facade adoption by the GUI and future HTTP/JSON consumers.

## Visual GUI reality

The GUI already has a substantial Calibre-like shell and a rich central `BookRow`, but it is strongly coupled to `caliberate-db` and source is highly concentrated.

At restart/current inspection:

- `crates/gui/src/views.rs` is roughly 493 KB;
- `crates/gui/src/app.rs` is roughly 65 KB;
- `crates/gui/src/preferences.rs` is roughly 68 KB;
- `BookRow` already carries title, authors, series, tags, format, rating, publisher, languages, cover presence, added/modified/pubdate, ID, and path;
- `LibraryView` directly stores `Database`, DB category values, and DB detail DTOs.

The immediate goal after `0009` is to begin moving visible browse/search/category-browser reads onto the common library service without rewriting the entire GUI.

The P0 visual target is recognizably Calibre-like information architecture: action toolbar, global/advanced search, left category/tag browser, central list/cover-grid browsing, optional cover browser, right book-details panel, virtual libraries, and persistent layout controls.

## Reader reality

The current GUI reader meaningfully loads only `txt`, `md`, and `markdown`. EPUB, PDF, DOCX, MOBI/AZW, and HTML are not real GUI reader loaders yet. Reader expansion remains P1 behind the visual library/service platform.

## TTS reality

No reader TTS implementation was found at restart. The future speech abstraction can therefore be introduced cleanly, but it is not the immediate P0 focus.

## Conversion reality

The conversion CLI and orchestration exist, but practical cross-format conversion remains largely unimplemented beyond passthrough behavior. Finished core conversion must not require a Calibre installation.

## Immediate work queue

1. `0009-library-book-summaries` — add an efficient rich library-domain summary page matching the central Calibre-like table/grid, using fixed-count batched metadata queries rather than N+1 lookups.
2. Begin migrating the visible Calibre-like GUI browse/search/category-browser read path onto the common library service.
3. Extend library sorting only as required by that GUI migration (authors/series/rating/date/etc.).
4. Add an HTTP/JSON adapter over the same service semantics.
5. Deepen library-source support: directory-backed and attached-Calibre modes.

## Completion standard

A roadmap checkbox, UI control, struct field, or stub does not establish feature completion. Completion requires executable behavior plus evidence appropriate to the feature: tests, runtime validation, or both.
