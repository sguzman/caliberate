# 0014 — Decouple LibraryCatalog from the Caliberate SQLite database

## Goal

Introduce the source/backend seam required for a headless Caliberate service to expose libraries that are **not** stored in Caliberate's own SQLite schema.

The immediate target after this task is a read-only adapter for an existing Calibre library folder containing `metadata.db` and Calibre-managed book files.

Today `LibraryCatalog<'a>` is hard-wired to:

```rust
db: &'a Database
```

and its methods directly invoke `caliberate_db::Database`.

That prevents the service from presenting an attached Calibre library without first importing/copying it into the Caliberate database.

This task creates only the reusable backend abstraction and preserves all existing behavior through a Database implementation.

Do **not** implement the Calibre adapter in this task.

## Architectural target

After this task the shape should be equivalent to:

```text
                LibraryCatalog
                      |
              library-domain trait
                /            \
               v              v
    Caliberate DB backend   future Calibre backend
```

The catalog API remains the reusable consumer-facing library service.

GUI, OPDS, future JSON HTTP, and sibling projects should not need to know which backend supplies the library.

## Scope

### 1. Add a library-domain backend/repository trait

Add one explicit trait in the `caliberate-library` crate, for example:

```rust
pub trait LibraryBackend {
    fn list_books(&self) -> CoreResult<Vec<LibraryBook>>;
    fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>>;
    fn search_books(&self, query: &str) -> CoreResult<Vec<LibraryBook>>;
    fn query_books(&self, query: &LibraryQuery) -> CoreResult<Vec<LibraryBook>>;
    fn query_page(&self, query: &LibraryQuery) -> CoreResult<LibraryQueryPage>;
    fn query_summary_page(&self, query: &LibraryQuery) -> CoreResult<LibrarySummaryPage>;
    fn list_facets(&self, kind: LibraryFacetKind) -> CoreResult<Vec<LibraryFacetValue>>;
    fn resolve_content(&self, book_id: i64) -> CoreResult<Option<LibraryContent>>;
}
```

Exact naming may follow repository style.

The trait must use **library-domain types only** at its public boundary.

Do not expose:

- `BookQuery`;
- `BookRecord`;
- `BookSummaryRecord`;
- raw `rusqlite` values;
- backend-specific SQL/path structures.

If some methods can have sensible default implementations over lower-level trait methods without weakening performance or semantics, that is acceptable, but do not over-engineer a generic framework.

### 2. Implement the trait for the existing Caliberate Database

Because the trait is local to `caliberate-library`, it may be implemented directly for:

```rust
caliberate_db::database::Database
```

or through a small wrapper if a wrapper has a concrete architectural benefit.

Preserve the exact current semantics of:

- list/get/search;
- structured queries;
- totals/pagination;
- summary pages;
- facets;
- content resolution priority:
  - copied asset;
  - first asset;
  - logical book path.

The existing `LibraryQuery -> BookQuery` conversion remains backend-specific implementation detail for the Database backend.

No behavior regression is allowed.

### 3. Make LibraryCatalog depend on the library-domain trait

Change `LibraryCatalog` so it no longer stores a concrete `&Database`.

A preferred compatible shape is equivalent to:

```rust
pub struct LibraryCatalog<'a> {
    backend: &'a dyn LibraryBackend,
}
```

with:

```rust
pub fn new(backend: &'a dyn LibraryBackend) -> Self
```

or a generic constructor that coerces to the trait object.

Existing call sites such as:

```rust
LibraryCatalog::new(&db)
```

should continue to compile when `db` is a Caliberate Database.

Do not make consumers construct DB-specific adapter plumbing merely to preserve existing behavior.

### 4. Catalog delegates; backend owns source semantics

After the change, `LibraryCatalog` should be thin library-service delegation/policy, not a place with concrete Caliberate SQL knowledge.

Database-specific mapping belongs in the Database backend implementation.

This separation is important because task 0015 will add a Calibre `metadata.db` implementation without changing every consumer.

### 5. Preserve all current consumers

No behavior change is expected in:

- GUI;
- OPDS/server;
- CLI paths using LibraryCatalog;
- tests using LibraryCatalog with Database.

Do not change visible GUI behavior.

Do not change OPDS wire output.

Do not add a JSON API yet.

### 6. Keep the abstraction read-only

This first backend seam covers the existing read/content surface only.

Do not add mutation methods.

The attached Calibre source is intended to start as read-only; overlay/write semantics are later work.

## Tests

Add focused library tests proving the seam is real rather than cosmetic.

At minimum prove:

1. `Database` satisfies the new backend trait and existing `LibraryCatalog::new(&db)` usage still works.
2. Existing list/get/search/query/page/summary/facet/content tests still pass.
3. Add a tiny fake/in-memory backend implemented **without Database** and pass it to `LibraryCatalog`.
4. Through that fake backend, prove at least:
   - list or get delegates correctly;
   - a structured query reaches the backend as a `LibraryQuery`;
   - content resolution can return a library-domain `LibraryContent`.
5. The fake backend test must not import or depend on DB-domain query/record types.

The fake backend is specifically evidence that a future Calibre adapter can exist behind the same catalog API.

Keep the fake compact; do not build an in-memory database framework.

## Explicit non-goals

Do **not**:

- implement reading Calibre `metadata.db`;
- scan a Calibre library folder;
- import Calibre books;
- mutate a Calibre source library;
- add source-selection config;
- add headless CLI flags for a Calibre folder;
- add HTTP JSON routes;
- change OPDS output;
- change GUI behavior;
- change sorting/filtering semantics;
- add schema migrations;
- add dependencies;
- generalize into plugins/dynamic loading;
- add async traits;
- add write/mutation methods;
- refactor unrelated crates.

If the seam appears to require widespread consumer changes, STOP and report why rather than performing a large refactor.

## Expected files

Primarily:

- `crates/library/src/catalog.rs`
- optionally one small new library module such as `backend.rs`
- `crates/library/src/lib.rs`
- focused library tests
- `docs/work/reports/0014.md`
- move this task to `docs/work/done/0014-library-backend-seam.md`

Small compile-only import adjustments in consumers are acceptable if required by public exports, but behavior changes are not.

No dependency or lockfile changes should be necessary.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-library
cargo test -p caliberate-server
cargo test -p caliberate-gui
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass. Existing unrelated GUI warnings may remain.

## Handoff

Write `docs/work/reports/0014.md` with:

- architecture introduced;
- exact trait surface;
- how Database implements it;
- consumer compatibility;
- files changed;
- validation actually run and results;
- risks/unverified behavior;
- deviations/blockers.

Move this task to:

- `docs/work/done/0014-library-backend-seam.md`

Commit and push exactly one bounded implementation branch:

- `codex/0014-library-backend-seam`

Do not work on any other task.
