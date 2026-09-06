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

## Attached Calibre backend — integrated

Task `0015-attached-calibre-backend` added a production read-only `CalibreLibraryBackend` behind `LibraryBackend`.

It opens `metadata.db` with SQLite read-only flags plus `PRAGMA query_only = ON`, validates the required modern Calibre base schema, implements the current library query/filter/sort/facet/summary surface directly against Calibre tables, and reconstructs source ebook paths without importing or mutating the source library.

The adapter uses the current temporary single-format projection by smallest `data.id`, normalizes format names to lowercase, bulk-loads page metadata, and validates source path components before resolving content.

Accepted commit: `6cb9fdcd59b7d9b96e2cc67be7b6aa5d3b49ae48`.

Synthetic Windows tests cover source-byte preservation, schema safety, path traversal protection, sort/filter parity, paging totals, summaries, facets, and content resolution. Real-library compatibility/performance is still pending human runtime verification.

## Headless attached Calibre server — integrated

Task `0016-headless-attached-calibre-server` added explicit server source selection and the CLI flag:

```text
calibre-server --calibre-library <PATH>
```

Existing OPDS list/get/search/download operations now route through the source-neutral `LibraryCatalog` seam. Attached Calibre content is authorized only inside the selected canonical library root and is canonicalized again before streaming, while the configured-Database external-reference policy remains unchanged.

Accepted commit: `a003eb9f6ecd241dd1fa905b40681014b45dd7d5`.

The next milestone is human runtime verification against the user's real full Calibre library. Do not queue another implementation task until that real-source check identifies either success or a concrete compatibility/performance defect.

## Real-library runtime defect — WSL/UNC SQLite locking

Human acceptance against the real Calibre source at `\\wsl$\Ubuntu\mnt\wsl\PHYSICALDRIVE0p1\calibre\en_nonfiction` reached the attached backend but failed during schema validation with SQLite `DatabaseBusy` / `database is locked`.

This is a filesystem/VFS locking compatibility defect at the native-Windows <-> WSL UNC boundary, not a Calibre schema mismatch.

Task `0016.1-wsl-immutable-calibre-access` is ready. It adds an explicit, opt-in immutable read-only mode for static attached Calibre sources. Normal locking read-only remains the default; immutable mode must never be silently selected because SQLite disables locking/change detection and assumes the source is not changing.

## WSL/UNC immutable Calibre access — integrated

Task `0016.1-wsl-immutable-calibre-access` added explicit immutable read-only SQLite URI mode for static attached Calibre sources behind filesystems where normal SQLite locking fails, such as the observed native-Windows access to a WSL UNC path.

Normal locking read-only remains the default. Immutable mode is opt-in via:

```text
--calibre-library-immutable
```

and uses:

```text
file:<encoded-path>?mode=ro&immutable=1
SQLITE_OPEN_READ_ONLY
SQLITE_OPEN_URI
PRAGMA query_only = ON
```

Accepted commit: `aca5f05da4cc13f2e10196d890bcfc8e89ea9d28`.

Human real-library acceptance should now retry the same WSL-backed library with the immutable flag, keeping the Calibre source static for the duration of the Caliberate process.

## Real-library runtime defect — SQLite URI authority on WSL UNC

Human acceptance of `0016.1` advanced past the original `DatabaseBusy` lock failure but failed opening the immutable URI for the real WSL source with:

```text
invalid uri authority: wsl%24
```

The current bundled SQLite rejects arbitrary non-localhost URI authorities, so a `file://wsl$/...` style URI cannot be used for this WSL UNC source.

Task `0016.2-windows-unc-static-calibre-access` is ready. It keeps local immutable URI behavior, but uses the Windows `win32-none` no-lock VFS on explicitly-selected static UNC/WSL sources, still with read-only flags and `PRAGMA query_only = ON`.

## Windows UNC static Calibre access — integrated

Task `0016.2-windows-unc-static-calibre-access` fixed the real WSL/UNC static-source open path.

For explicitly-static attached Calibre sources:

- normal local Windows paths keep SQLite immutable URI mode;
- Windows UNC/WSL paths use the ordinary UNC filesystem path with `SQLITE_OPEN_READ_ONLY` and the built-in `win32-none` VFS;
- both still execute `PRAGMA query_only = ON`.

Normal locking read-only remains the default and no automatic fallback was added.

Accepted commit: `ee4044d29effff6edb4fc2ccacd3c73f40035fe1`.

Human real-library acceptance should now retry the same WSL-backed library with `--calibre-library-immutable`, keeping the source static for the duration of the process.

## Real full-library headless acceptance — passed

Human Windows runtime acceptance against the user's actual WSL-backed Calibre library succeeded end to end.

Observed real source:

```text
\\wsl$\Ubuntu\mnt\wsl\PHYSICALDRIVE0p1\calibre\en_nonfiction
```

with explicit static-source mode.

Acceptance evidence:

- `check-config` completed successfully against the real `metadata.db`;
- the headless server bound successfully at `127.0.0.1:8181`;
- `/health` returned HTTP 200 / `ok`;
- OPDS search for `Romanovs` returned real book ID `56016`, title `The Last Days of the Romanovs`;
- `/opds/books/56016` returned the expected real entry and acquisition link;
- `/opds/books/56016/download` streamed the actual ebook successfully;
- the downloaded test artifact size was 354,595 bytes.

This establishes that the source-neutral service and attached-Calibre server work against the user's real full library without importing the source into Caliberate first.

The next service limitation is the single-primary-format compatibility projection.

## Library all-format service — integrated

Task `0017-library-all-formats` extended the source-neutral library domain with additive per-book all-format APIs:

- `LibraryFormat`;
- `LibraryCatalog::list_formats(book_id)`;
- `LibraryCatalog::resolve_content_format(book_id, format)`.

Attached Calibre sources now expose every `data` row in deterministic `data.id` order, normalize format names, report valid `uncompressed_size` values, deduplicate malformed case-only duplicates by lowest row ID, and resolve a requested format through the existing safe source-path policy.

The configured Caliberate database intentionally exposes only its canonical `books.format` because its current schema does not model true logical multi-format identity.

Existing primary-format fields and `resolve_content(book_id)` semantics remain unchanged for GUI/OPDS compatibility.

Accepted commit: `50b0b5213fdd6fc4faa0d408c5920843f5d8ce23`.

## Versioned HTTP/JSON library API — integrated

Task `0018-http-json-library-api` added the first general-purpose, source-neutral programmatic API under:

```text
/api/v1
```

The API now exposes bounded browsing, structured queries, compatibility search, book detail, all-format discovery, primary and format-specific content streaming, and facets.

Protocol DTOs are separate from library-domain structs and JSON metadata responses do not expose filesystem/source paths. Browse/query defaults are bounded to 100 items with a maximum of 500.

OPDS and JSON now share one server-internal content authorization/canonicalization/streaming policy, preserving attached-root containment, configured external-reference rules, download enablement, size limits, and MIME mapping.

Synthetic router tests cover both configured-Database and attached-Calibre sources, including attached source isolation via a `must-not-open.db`, two-format PDF/EPUB behavior, case-insensitive format requests, primary-format compatibility, auth, URL prefixes, JSON error envelopes, and source-byte preservation.

Accepted commit: `55ee7c70326e0614dc8b7cfdd71252c7da6b78bf`.

The next milestone is human runtime acceptance of the JSON API against the real WSL-backed Calibre library before further protocol expansion.

## Real full-library JSON API acceptance — passed

Human Windows runtime acceptance of the versioned JSON API succeeded against the actual WSL-backed attached Calibre library containing 105,570 books.

Observed real acceptance:

- bounded browse:
  - `GET /api/v1/books?limit=3&sort=title&direction=asc`
  - returned `total = 105570`, `offset = 0`, `limit = 3`;
  - returned rich metadata without filesystem paths;
- structured query for `Romanovs` returned exactly one real match:
  - ID `56016`;
  - title `The Last Days of the Romanovs`;
  - author `N. Sokolov`;
  - primary format `epub`;
- book detail returned the expected versioned self/content hrefs;
- format discovery returned the real EPUB format with `size_bytes = 354595`;
- primary JSON content streaming produced a 354,595-byte ebook;
- format-specific `/content/epub` streaming produced the same 354,595-byte payload.

This establishes the JSON API as a working external-consumer interface over the real full attached Calibre corpus.

The tested book currently has one stored format, so real multi-format source acceptance remains to be exercised with another real book when convenient.

## OPDS multi-format acquisition — integrated

Task `0019-opds-multi-format` upgraded the OPDS adapter to expose alternate stored formats while preserving the legacy primary acquisition route.

Existing primary behavior remains:

```text
/opds/books/{id}/download
```

Alternate formats are now exposed as additional acquisition links and streamed through:

```text
/opds/books/{id}/download/{format}
```

The OPDS adapter uses `LibraryCatalog::list_formats` / `resolve_content_format`, preserves service format order, excludes the primary format from duplicate alternate links, honors URL prefixes/authentication, and delegates all bytes through the same shared content authorization/canonicalization/streaming policy used by JSON.

Synthetic configured-Database and attached-Calibre tests cover canonical route byte parity, unavailable formats, PDF primary + EPUB/MOBI alternate ordering, case-insensitive resolution, source isolation, unchanged metadata bytes, auth, and prefix behavior.

Accepted commit: `3ea85128cf29b117a0d7e49568094c71f22e4b08`.

Real multi-format OPDS acceptance is still pending against an actual attached book with more than one stored format.

## Real multi-format discovery attempt — N+1 gap exposed

Human runtime acceptance attempted to locate a real multi-format book in the attached 105,570-book Calibre corpus using the existing JSON API.

The first 5,000 books were scanned in 500-book pages. No multi-format specimen was found in that sample.

The important result is architectural: each page still required a separate `/books/{id}/formats` request per book, so continuing across the full corpus would require roughly 105k additional HTTP requests.

This is not treated as an OPDS failure. Synthetic task `0019` coverage already proves multi-format protocol behavior.

Task `0020-batched-summary-formats` is ready to eliminate this N+1 discovery gap by projecting all formats into bounded summary pages using page-level backend batching.

## Canonical catalog ownership clarification

The product model is now explicit:

- Caliberate's own SQLite database is the canonical mutable catalog for a maintained library.
- Existing Calibre libraries are external sources/provenance, not the permanent metadata authority.
- Calibre metadata should be materialized into the Caliberate DB while legacy ebook files may remain read-only external references.
- New native books can coexist with imported legacy books in the same canonical catalog.
- Logical book identity, logical format identity, and physical storage representation are separate concepts.
- Existing `assets` copy/reference/compression machinery is retained and extended rather than replaced.
- Future storage may include managed compressed/archive representations resolved on demand.

Durable architecture: `docs/project/library-ownership-and-storage.md`.

Task `0021-canonical-provenance-formats` is the first implementation step: source provenance tables, canonical logical-format rows, and format-aware physical assets. The following task should materialize an attached Calibre source into the canonical DB without copying ebook files.

## Batched summary formats — integrated

Task `0020-batched-summary-formats` extended `LibraryBookSummary` with all stored formats and made attached-Calibre summary pages load formats through bounded page-level SQL batches rather than one query/request per book.

Attached format loading uses parameterized ID chunks of 400 and preserves source `data.id` order, lowercase normalization, safe size conversion, and case-only deduplication. Managed-DB summaries remain canonical one-format until the canonical logical-format foundation lands.

JSON browse/query summaries now expose additive `format_count` and path-free `formats` entries.

Accepted commit: `c27af97a3fa1a9fc7d79fee043cc3803553d07c5`.

## Canonical provenance and logical formats — integrated

Task `0021-canonical-provenance-formats` made the Caliberate-owned SQLite database explicitly represent external source provenance, canonical logical formats, and format-aware physical assets.

Implemented foundations:

- `library_sources`;
- `source_books`;
- `book_formats`;
- nullable `assets.book_format_id` / `assets.source_id`;
- schema 10 -> 11 migration/backfill for existing managed libraries;
- stable source/source-book APIs;
- batched logical-format loading;
- compatibility `add_asset` auto-linking without filename inference;
- explicit format-aware asset insertion;
- managed `LibraryBackend` multi-format summaries and format-specific resolution;
- canonical deletion cleanup that retains source registry rows.

The task-specific DB implementation lives in `crates/db/src/database/canonical.rs` rather than further growing the historical DB god file.

Accepted commit: `15ce9ac84fade39653243beda1651cdf86dced20`.

## Calibre canonical materialization — integrated

Task `0022-calibre-materialization` added a resumable offramp from direct attached-Calibre operation into the Caliberate-owned canonical database.

Implemented behavior:

- keyset-paged Calibre source reads by `books.id`;
- hard-bounded public page size `1..=500`;
- batched relation and format loading with bounded ID chunks;
- one canonical target SQLite transaction per source page;
- canonical book/metadata/source-book/format/reference-asset materialization;
- metadata-derived safe Calibre reference paths without ebook filesystem scanning;
- repeat import skips existing `(source_id, external_id)` mappings and preserves local canonical edits;
- partial committed pages resume safely;
- source completion timestamp updates only after a full successful pass;
- `calibredb import-calibre --source ... --database ... [--immutable]`.

Accepted commit: `b4222e733e6c9e76086db87825013662f36f9cd6`.

Real full-corpus materialization into a local Caliberate DB is the next human acceptance gate.

## Materialized local-catalog hybrid acceptance — passed

Human Windows runtime acceptance proved the Calibre -> canonical Caliberate offramp end to end against the actual 105,570-book corpus.

Real materialization result:

```text
source_id=1
seen=105570
imported=105570
skipped_existing=0
metadata_only=0
logical_formats=106949
reference_assets=106949
last_external_id=107655
completed=true
```

Canonical DB:

```text
A:\Data\Books\db\caliberate.sqlite
```

The managed-Database CLI then opened that local DB directly, without attached-Calibre mode, and found `The Last Days of the Romanovs` as canonical Caliberate book ID `53937`.

The headless server was launched with no `--calibre-library` argument and reported:

```text
source=configured database
```

JSON search returned canonical ID `53937`; its EPUB format reported `size_bytes=354595`; and `/api/v1/books/53937/content` streamed exactly 354,595 bytes from the legacy reference asset.

This proves the intended hybrid state:

```text
Caliberate-owned local SQLite -> catalog/query/API identity
legacy Calibre tree           -> physical reference bytes only
```

Calibre's `metadata.db` is no longer required in the ordinary runtime catalog path for the materialized library.

Managed compressed-content serving is now implemented: `LibraryContent`
distinguishes logical format from physical encoding and the server streams
zstd copies as decoded logical bytes with logical-size download limits. Do not
migrate real legacy content until the explicit adoption task is implemented.

## Transparent managed compressed-content serving — integrated

Task `0023-transparent-compressed-content` separated logical ebook format from physical storage encoding and made zstd-managed copies transparently consumable through the existing content service.

Implemented behavior:

- `LibraryContentEncoding::{Identity,Zstd}`;
- logical and stored size propagation;
- managed compressed copies remain preferred through the existing copy-before-reference rule;
- attached-Calibre content remains identity encoded;
- shared HTTP content streaming authorizes the physical path first, then asynchronously zstd-decodes without whole-file buffering;
- download limits are checked against known logical decoded size for compressed assets;
- JSON/OPDS wire shapes remain unchanged and continue returning original logical ebook bytes;
- corrupt preferred zstd streams terminate as body errors without panic or silent fallback.

Accepted commit: `6869883e6b26ff5cfb134659380b2bfd0962bb08`.

This clears the representation seam required before progressive legacy-content adoption.

## Single-format reference adoption — integrated

Task `0024-adopt-reference-format` added explicit single-format adoption from
an external reference into a content-addressed Caliberate-managed object.
Adoption is additive: the managed copy is preferred while the original
reference and source provenance remain available as fallback. Objects are
SHA-256 addressed under `objects/sha256/<prefix>/`, optionally zstd-compressed
using existing asset policy, verified before asset registration, and reused
idempotently. Bulk adoption, source retirement/resync, reference deletion, and
pack/chunk storage remain future work.

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

1. Add explicit per-book/per-format legacy-reference adoption into Caliberate-managed storage while retaining the legacy reference as fallback.
2. Human-adopt one real legacy book and prove the server continues serving identical logical bytes from the managed representation.
3. Then add source resync/reconciliation and source-retirement auditing, followed by deeper pack/chunk storage experiments.

## Completion standard

A roadmap checkbox, UI control, struct field, or stub does not establish feature completion. Completion requires executable behavior plus evidence appropriate to the feature: tests, runtime validation, or both.
