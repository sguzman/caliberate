# Library Platform Architecture

This document refines `ARCHITECTURE.md` for the current P0 visual-library/service direction.

## Core rule

**Database, GUI, and network protocols are not peers.**

The dependency direction should be:

```text
SQLite / Calibre metadata.db / directory scan
                |
                v
        library repositories
                |
                v
        library domain/service
        /        |         \
       v         v          v
    egui GUI   HTTP JSON   OPDS
                 |
                 v
            other projects
```

The library service owns application semantics. GUI and protocols translate user/wire requests into service calls.

## Why this matters now

The current server's OPDS handlers open `caliberate_db::Database` directly. That is acceptable scaffolding, but it means the protocol layer currently owns query/content behavior that other consumers cannot reuse cleanly.

The current GUI also contains substantial library behavior in large view modules. P0 work should steadily move shared semantics downward rather than building another parallel implementation for HTTP or future consumers.

## Domain-facing types

Exact Rust names can evolve, but the read surface should converge on format-independent, frontend-independent concepts.

### LibraryDescriptor

- stable library id;
- display name;
- source kind;
- source/root description safe to expose;
- capabilities such as writable/read-only/rescan support.

### BookSummary

Optimized for collection/table/grid views:

- stable logical book id;
- title;
- authors;
- series/index when present;
- rating;
- tags/categories needed by common views;
- format summary;
- cover availability/key;
- dates/size fields needed for sort/display;
- selected custom display fields where configured.

Do not load full comments, giant metadata blobs, or book bytes for every row/card.

### BookDetails

Selected-book metadata including:

- full common metadata;
- identifiers;
- comments/description;
- format list;
- cover descriptor;
- custom fields;
- source/library information appropriate for UI/API display.

### BookFormat / ContentDescriptor

Represent a usable book payload without requiring consumers to know storage layout.

Possible fields:

- format id/name;
- media type;
- size;
- logical filename;
- content identity/version information;
- access method internal to service implementation.

External protocols receive a stream/response, not an arbitrary filesystem path unless an explicit trusted in-process API requires it.

### LibraryQuery

One common query model should eventually express:

- free text;
- field restrictions;
- category/facet restrictions;
- virtual/saved search restriction;
- sort keys/direction;
- pagination;
- library/source selection.

Frontends may have richer UI syntax, but they should compile to the shared query model/search engine.

### QueryResult

- deterministic ordered `BookSummary` page;
- total/estimated count where practical;
- pagination continuation metadata;
- effective query/restriction metadata useful for clients.

### Facet/category types

Support the Calibre-like Tag browser and remote filtering:

- category kind/name;
- values;
- counts;
- optional hierarchy;
- average-rating/popularity metadata only if cheap/useful.

## Service split

Prefer coherent interfaces over one enormous god trait.

A likely shape is conceptually:

```text
LibraryRegistry      -> list/open libraries and source capabilities
LibraryQueryService  -> query books, details, categories, saved searches
LibraryContent       -> covers and book-format content
LibraryMutation      -> later metadata/import/remove/rescan operations
```

This is a conceptual boundary, not a command to create exactly four Rust traits immediately. Start with the smallest service seam that removes duplicated frontend/protocol logic.

## Storage/source adapters

### Managed Caliberate source

Uses Caliberate's own SQLite/storage/assets implementation.

### Directory source

Scanner/indexer populates Caliberate-owned index/state while source files remain externally owned.

### Calibre source

Compatibility adapter reads Calibre `metadata.db` and resolves Calibre's directory layout. Generic consumers must not contain Calibre SQL/table names.

## GUI architecture

The visual library UI should consume `BookSummary`/query/facet data and maintain only presentation state:

- selected book ids;
- column/layout preferences;
- cover-grid dimensions;
- open/closed panes;
- current query/search text;
- scroll/selection state;
- transient dialogs/context menus.

It should not own SQL, source-specific path rules, OPDS concepts, or protocol concerns.

## Calibre visual model

P0 UI follows Calibre's information architecture because it is dense and proven for large ebook libraries:

```text
+---------------------------------------------------------------+
| toolbar actions                                               |
+---------------------------------------------------------------+
| search / advanced search / virtual library                    |
+----------------+------------------------------+---------------+
| category/tag   | book list OR cover grid      | book details  |
| browser        |                              | + cover        |
|                | optional cover browser       |               |
+----------------+------------------------------+---------------+
| status / jobs / layout controls                               |
+---------------------------------------------------------------+
```

Exact styling can diverge, but major panes, visibility toggles, and workflows should remain recognizable.

## Protocol architecture

### In-process Rust

The GUI should ideally exercise the same public-ish service API that sibling Rust projects can use.

### HTTP/JSON

Version the external API from the beginning (`/api/v1/...` or equivalent). Wire types may differ from internal Rust types but should map directly to domain concepts.

### OPDS

OPDS is a representation/acquisition adapter. It should not become the canonical search implementation.

### Future protocols

Additional adapters should be thin enough that implementing one mostly means:

1. parse/authenticate request;
2. map to library service operation;
3. serialize/stream result.

If a new adapter requires duplicating query, content-selection, or source-specific logic, the service boundary is incomplete.

## Performance expectations

Visual browsing and remote consumption require treating large libraries as normal.

- never fetch all covers or full metadata synchronously for a table refresh;
- query/paginate deterministically;
- lazy-load covers/details;
- cache thumbnails rather than repeatedly decoding originals;
- stream book payloads;
- avoid opening/reinitializing expensive database state per individual row/request when a reusable service/session can own it safely;
- instrument slow queries and scans with tracing.

## Mutation safety

Read/query/content APIs come first.

Later mutations must respect source capabilities:

- managed library: writable;
- arbitrary external directory: only explicit file operations, otherwise overlay/index mutations;
- attached Calibre library: read-only initially;
- writable Calibre compatibility: separate opt-in milestone after interoperability/corruption tests.

## Architectural smell tests

Stop and reconsider a patch if:

- an Axum handler imports raw DB tables/queries for generic library behavior;
- egui code opens SQLite directly for reusable library operations;
- OPDS and JSON APIs implement separate search semantics;
- a consumer needs physical file paths just to retrieve book content;
- a new library source requires rewriting GUI search/browse logic;
- adding a protocol requires duplicating domain behavior.
