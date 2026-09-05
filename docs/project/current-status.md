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

Task `0001-windows-path-identity` fixed the failing `ebook_convert_rejects_input_equals_output` case by comparing canonical paths when the output already exists.

Accepted commit: `3bbc5f10a45ec68ab9f4ff8f556432c44cae1268`.

## Cross-platform CI — integrated

Task `0002-cross-platform-ci` added `.github/workflows/cross-platform-ci.yml` with a Windows/Linux GitHub Actions matrix. Each job runs formatting, locked workspace checking, and locked workspace tests. Subsequent hosted `main` runs have demonstrated that the workflow is active; individual future run status should still be checked before making a claim about that run.

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

## Rich library book summaries — integrated

Task `0009-library-book-summaries` added the read model needed by the central Calibre-like list/grid:

- `BookSummaryRecord` in the DB layer;
- page-wide batched loading for authors, tags, series/index, publisher, rating, languages, cover presence, and dates;
- `LibrarySeriesSummary`;
- `LibraryBookSummary`;
- `LibrarySummaryPage`;
- `LibraryCatalog::query_summary_page`.

The summary loader preserves base query order and uses a fixed set of metadata queries for the requested page rather than calling per-book metadata getters in the result loop. GUI callers should keep summary page sizes bounded because the batch queries bind the page's book IDs in `IN (...)` clauses.

Accepted commit: `929456348f4c1471ae6bdb7c8875b6ceff0577fd`.

## Visible GUI library-service read path — integrated

Task `0010-gui-library-service-read-path` moved the actual visible library browse read path onto the common library service:

- central table/grid/shelf rows are built from `LibraryBookSummary` values returned by bounded `LibraryCatalog::query_summary_page` chunks;
- summary chunks are capped at 500 books and loaded in deterministic ID order while the GUI still requires a full in-memory working set;
- the main Authors/Tags/Series/Publishers/Ratings/Languages browser lists now use `LibraryFacetValue` returned by `LibraryCatalog::list_facets`;
- non-empty All-search candidate IDs now come through `LibraryCatalog::search_books`;
- tag/language/publisher autocomplete values reuse the service-backed facets;
- the old `MetadataCache` dependency and per-book `build_row` enrichment path were removed from `LibraryView`.

Details, editing, mutations, management dialogs, saved-search persistence, device/news flows, advanced include/exclude filtering, and richer GUI sorts deliberately remain outside this first read-path migration.

Native Windows cargo validation passed, including GUI tests and the workspace suite. Luna could not perform an interactive desktop smoke test in its environment, so runtime GUI interaction after this migration remains to be checked manually on the user's Windows desktop.

Accepted commit: `84dcf6d88674e515425367ffd573e173c80b42b4`.

## GUI pane acceptance — functionally accepted with deferred edge-case polish

Tasks `0010.1`, `0010.2`, and `0010.2.1` moved the Library into the central surface, added resizable/collapsible Browser and Details panes, aligned pane config validation with the 200 px runtime floor, separated preferred widths from transient runtime clamping, and added emergency narrow-window suppression.

Human Windows testing confirmed the layout is functional enough to move on. One narrow-window visual edge case can still appear under aggressive resizing; this is explicitly deferred rather than allowed to block higher-priority library-platform work.

## Calibre smoke bootstrap — functionally accepted

Tasks `0010.3` and `0010.3.1` corrected the development `en_nonfiction` smoke workflow:

- Calibre-style `Title - Author` filenames are interpreted with the last separator;
- the smoke environment uses its own dedicated SQLite database;
- GUI startup is pinned to that dedicated database;
- reset is scoped to dedicated dev DB artifacts;
- positive file limits use bounded discovery and progress is visible;
- human Windows verification confirmed Title and Authors are no longer globally reversed.

Two low-priority issues are intentionally deferred:

- discovery over the WSL-backed source can still take several minutes before indexing starts;
- some individual author strings remain messy and need source-aware metadata quality work before assigning blame to Caliberate.

These do not currently block the library-platform roadmap.

## Library sort parity — integrated

Task `0011-library-query-sort-parity` extended the structured DB/library sort layer to Authors, Series, Tags, Rating, Publisher, Languages, DateAdded, DateModified, and PubDate while preserving deterministic ID tie-breaking and summary-page order.

Accepted commit: `f1334d1f492ddb6c11c6f8dbe496c02aed76d96f`.

## Library filter parity — integrated

Task `0012-library-query-filter-parity` added typed ANDed Include/Exclude metadata predicates for Authors, Tags, Series, Publishers, Ratings, and Languages at the DB/library-service boundary.

Structured string filters use literal case-insensitive substring semantics with SQL LIKE metacharacters escaped; Ratings use exact numeric equality. Count and result queries share the same predicate construction.

Accepted commit: `b852ff149915ba5315aaa3c19cd152068d9ae1e8`.

## GUI service browser filters — integrated

Task `0013-gui-service-browser-filters` moved visible Authors/Tags/Series/Publishers/Ratings/Languages Include/Exclude row filtering onto `LibraryQuery::metadata_filters`.

The GUI now loads the complete service-browser-filtered working set in bounded summary chunks; `apply_filters()` no longer re-evaluates browser filters against formatted `BookRow` strings. Browser saved-search state is validated before the service query is built, preventing stale filter/UI divergence.

Accepted commit: `ba77dbf9ab6eecb7741c615a30ceb1deaf210f8c`.

Native Windows automated validation passed. Interactive Windows GUI verification is still pending.

## Product direction pivot — headless services first

Human Windows verification confirmed the integrated 0013 browser-filter path works correctly.

The GUI is now frozen except for service-blocking regressions. Near-term work pivots to the reusable/headless service platform.

Primary target:

> Run Caliberate headlessly against an entire existing Calibre library folder, without a Calibre process and without importing/mutating the source library, then expose that library to other applications through reusable APIs.

The immediate architectural blocker is that `LibraryCatalog` is still concrete over `caliberate_db::Database`. Task `0014-library-backend-seam` introduces a source-neutral read backend so a future attached-Calibre `metadata.db` backend can sit behind the same catalog API.

## Library backend seam — integrated

Task `0014-library-backend-seam` decoupled `LibraryCatalog` from the concrete Caliberate SQLite database. `LibraryCatalog` now delegates to the source-neutral read-only `LibraryBackend` trait, while the existing `Database` implements that trait without changing current GUI/server consumers.

Accepted commit: `6bd09c292d5c3edcea1a2349bc0623d3e5b953c6`.

A fake non-Database backend test proves future source adapters can use the same library-domain catalog API.

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

The reusable read-only library-domain facade now exists for:

- basic catalog list/get/search;
- content resolution;
- structured filtering;
- category facets;
- deterministic sorting by ID/title/format;
- database-backed pagination and filtered totals;
- rich batched book-summary pages.

OPDS list/get/search/download storage selection consume the library facade. The visible GUI book-row and category-browser read path now consumes the same service. Protocol-specific authorization and wire behavior remain in the server.

Still not first-class:

- remaining local Format/News/search/sort/group semantics needed before universally correct page-at-a-time browsing;
- true service-backed GUI pagination;
- arbitrary directory-backed libraries with persistent rescan/reconciliation while leaving files in place;
- flat-directory source workflow;
- attached existing Calibre library with Calibre absent;
- clean separation between externally owned source data and Caliberate overlay state;
- HTTP/JSON consumers over the same service.

## Visual GUI reality

The GUI already has a substantial Calibre-like shell and rich presentation behavior, but `crates/gui/src/views.rs` remains a large DB-coupled god file.

Current state:

- the main visible book-row source is now `LibraryCatalog::query_summary_page`, not direct `Database::list_books` plus `MetadataCache` enrichment;
- the main category browser now uses library-domain facet values;
- `BookRow` remains a GUI presentation type that formats structured summary data for the table/grid/shelf;
- the GUI still maintains a full `all_books` working set in memory because advanced include/exclude filters, many sorts, grouping, stats, and reader-library-search are still local;
- service sort support currently covers only ID/title/format, so paging cannot yet replace the full working set without breaking the other visible sort modes;
- details/editing/mutations/device/news/management dialogs remain directly DB-backed and are deliberately outside this read-path phase.

The next work is specifically to close the remaining query-semantic gap rather than add fake pagination over a locally filtered page.

The P0 visual target remains recognizably Calibre-like information architecture: action toolbar, global/advanced search, left category/tag browser, central list/cover-grid browsing, optional cover browser, right book-details panel, virtual libraries, and persistent layout controls.

## Reader reality

The current GUI reader meaningfully loads only `txt`, `md`, and `markdown`. EPUB, PDF, DOCX, MOBI/AZW, and HTML are not real GUI reader loaders yet. Reader expansion remains P1 behind the visual library/service platform.

## TTS reality

No reader TTS implementation was found at restart. The future speech abstraction can therefore be introduced cleanly, but it is not the immediate P0 focus.

## Conversion reality

The conversion CLI and orchestration exist, but practical cross-format conversion remains largely unimplemented beyond passthrough behavior. Finished core conversion must not require a Calibre installation.

## Immediate work queue

1. `0015-attached-calibre-backend` — implement the read-only modern Calibre `metadata.db` backend behind `LibraryBackend`, including query/filter/sort/facets and source file resolution.
2. Add headless source selection so `calibre-server` can serve an attached Calibre library directly.
3. Expose all Calibre formats per logical book instead of the current single-format compatibility projection.
4. Add an HTTP/JSON API over the same library-domain service semantics; keep OPDS as a parallel adapter.
5. Exercise the user's complete Calibre library headlessly and harden performance/content streaming.
6. Return to GUI work only when a service capability needs a visible client or a regression blocks usage.

## Completion standard

A roadmap checkbox, UI control, struct field, or stub does not establish feature completion. Completion requires executable behavior plus evidence appropriate to the feature: tests, runtime validation, or both.
