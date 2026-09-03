# Visual Library Platform Roadmap

This roadmap is the current P0 execution direction. It takes precedence over reader-first sequencing in older restart notes when they conflict.

## Goal

Make Caliberate useful first as a visual, standalone ebook library database and service platform:

- Calibre-like desktop browsing/searching;
- reusable library/query/content core;
- multiple library source types;
- multiple protocol/front-end adapters over the same semantics.

The GUI is a client of the library platform, not the library platform itself.

## V0 — Baseline and architecture seams

- [x] Native Windows workspace builds and GUI launches.
- [x] Fix confirmed Windows same-path conversion regression.
- [ ] Add Windows + Linux CI baseline.
- [ ] Inventory direct DB usage from GUI/server/CLI that should move behind a library service.
- [ ] Define read-only library/query/content facade before adding more protocol endpoints.

## V1 — Read-only library service

Create a stable application-facing service for reading library state.

Minimum concepts:

- library identity/source kind;
- `BookSummary` for collection views;
- `BookDetails` for selected-book details;
- `BookFormat` / content descriptor;
- query/filter/sort/pagination request;
- category/facet values and counts;
- cover/content resolution;
- virtual/saved search representation where existing DB capabilities permit it.

Properties:

- no egui types;
- no Axum/HTTP types;
- no OPDS XML types;
- no raw SQLite rows exposed to consumers;
- deterministic ordering and pagination;
- tracing around expensive queries/errors.

Exit gate: GUI/server/CLI can begin consuming one common read API.

## V2 — Library sources

Make library identity independent from one storage mode.

### Managed library

- current Caliberate DB/storage remains supported;
- multiple formats per logical book;
- covers and metadata resolve through the service.

### Directory-backed/reference library

- arbitrary directory tree;
- flat directory is valid;
- source files stay in place;
- index metadata/state separately;
- rescan/reconcile additions/removals/moves where practical.

### Attached Calibre library

- recognize library root and `metadata.db`;
- read/index through isolated compatibility adapter;
- no Calibre executable/process required;
- source mutation disabled initially;
- Caliberate-specific overlay state stored separately.

Exit gate: the same library service can expose books from all supported source types.

## V3 — Calibre-like desktop library shell

Recreate Calibre's useful desktop information architecture in egui.

### Main layout

- top action toolbar;
- global search row;
- left category/tag browser;
- central collection view;
- right book-details panel;
- bottom/status/layout controls where useful.

### Central collection modes

- detailed table/list view;
- cover grid;
- cover browser / horizontal visual browse mode;
- bookshelf/grouped mode later if it remains useful.

### Interaction

- persistent column/layout preferences;
- sort by visible metadata;
- multi-select;
- keyboard navigation;
- context actions;
- double-click/open/view behavior;
- drag/drop only where ownership semantics are explicit.

The target should feel recognizably like Calibre's library workflow even if visual styling is cleaner/different.

Exit gate: a user can browse a real library visually without dropping to CLI/database tools.

## V4 — Search, facets, and virtual libraries

Build search once in the library service and project it into every frontend.

- common metadata fields;
- boolean/fielded search;
- category/tag-browser generated restrictions;
- advanced-search builder;
- saved searches;
- virtual libraries;
- category/facet counts;
- search + sort + pagination composition;
- large-library performance tests.

GUI, HTTP API, OPDS, and CLI should agree on result semantics.

Exit gate: visual browsing and remote browsing produce the same library subsets for equivalent queries.

## V5 — Service adapters

### In-process Rust API

This is the primary contract for Caliberate GUI and sibling Rust projects.

- read library list/source metadata;
- query books;
- fetch details/categories;
- resolve covers/formats/content;
- later mutations behind explicit APIs.

### HTTP/JSON API

Expose the service using versioned resource-oriented endpoints.

Initial surface should prioritize other-project consumption:

- libraries;
- books query/list;
- book details;
- categories/facets;
- covers;
- formats/content downloads;
- health/version/capabilities.

Avoid encoding GUI-specific state in the API.

### OPDS

Refactor current OPDS handlers to call the service instead of opening `Database` directly.

Support browsing/search/acquisition from the common query/content model.

### Additional protocol adapters

Do not guess prematurely. Add WebDAV or other protocols when a real consumer needs them, preserving the same service boundary.

Exit gate: at least the GUI, HTTP JSON API, and OPDS use the same read/query/content semantics.

## V6 — Mutation service

Once read semantics are stable, expose controlled mutations:

- metadata edits;
- add/remove formats;
- tags/categories/series/custom fields;
- covers;
- add/remove/import books;
- rescan/reconcile;
- saved searches/virtual libraries.

Mutation semantics must respect library-source ownership. An attached external/Calibre source must not become writable merely because a generic mutation exists.

## V7 — Consumer hardening

Treat Caliberate as infrastructure for other projects.

- stable/versioned API contracts;
- capability discovery;
- pagination/cursors or deterministic offset pagination;
- clear errors;
- concurrency/locking policy;
- integration tests against large fixture libraries;
- protocol contract tests;
- documented sample clients;
- streaming for covers/content rather than unnecessary whole-file loads.

## P1 follow-on

After the platform is useful:

- normalized document model;
- real reader formats;
- TTS;
- deeper conversion;
- richer metadata providers/embedding;
- annotations/read-state synchronization.

These should consume the library service rather than bypass it.

## Explicitly deferred

Unless required by P0 work:

- device integration expansion;
- ebook editor/polishing;
- news recipes/acquisition;
- email delivery;
- plugin ecosystem expansion;
- writable Calibre compatibility.
