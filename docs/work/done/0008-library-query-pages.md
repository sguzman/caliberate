# 0008 — Database-backed library sorting, pagination, and totals

## Goal

Extend the structured library query seam from task `0007` with deterministic sorting, database-backed pagination, and total-count semantics suitable for the visual library browser and future HTTP/JSON API.

This task must keep large-library behavior sane: **do not fetch all matching books and then sort/slice/count in Rust.** Sorting, limit/offset, and filtered counting belong in the database query path.

The public consumer API must remain library-domain types. GUI/server callers must not need to import DB query types.

## Existing baseline

Current database behavior:

- `caliberate_db::query::BookQuery` supports structured filters plus `limit`.
- `Database::search_books_query(&BookQuery)` builds one `SELECT DISTINCT ...` query, currently hardcodes `ORDER BY b.id`, and optionally adds `LIMIT`.
- Query filters can join authors, tags, series, publishers, languages, and identifiers.

Current library behavior:

- `caliberate_library::query::LibraryQuery` mirrors the structured filters and `limit`.
- `LibraryCatalog::query_books(&LibraryQuery)` maps to the DB query.
- Facets are already library-domain values.

## Scope

### 1. Add DB sort and offset query semantics

In `crates/db/src/query/mod.rs`, add a small database query sort enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookSortField {
    Id,
    Title,
    Format,
}
```

Extend `BookQuery` with:

```rust
pub offset: Option<usize>,
pub sort: BookSortField,
pub descending: bool,
```

`BookQuery::default()` / `BookQuery::new()` must preserve current behavior:

- sort by ID;
- ascending;
- no offset.

Add builders:

```rust
pub fn with_offset(self, value: usize) -> Self;
pub fn with_sort(self, field: BookSortField) -> Self;
pub fn descending(self) -> Self;
```

Existing builders and existing query behavior must remain compatible.

### 2. Make DB query ordering deterministic and safe

Update `Database::search_books_query` so sorting is selected only by matching `BookSortField` to hard-coded SQL expressions.

Required mappings:

- `Id` -> `b.id`
- `Title` -> `b.title COLLATE NOCASE`
- `Format` -> `b.format COLLATE NOCASE`

Do **not** interpolate caller-provided strings into the ORDER BY clause.

For `Title` and `Format`, add `b.id ASC` as a deterministic tie-breaker regardless of primary sort direction.

Examples:

- title ascending: `ORDER BY b.title COLLATE NOCASE ASC, b.id ASC`
- title descending: `ORDER BY b.title COLLATE NOCASE DESC, b.id ASC`
- ID descending: `ORDER BY b.id DESC`

### 3. Add database-backed offset pagination

Support these cases in `Database::search_books_query`:

- limit only -> `LIMIT ?`
- limit + offset -> `LIMIT ? OFFSET ?`
- offset without limit -> `LIMIT -1 OFFSET ?`

Use SQL parameters for numeric values.

Do not fetch all rows and slice them in Rust.

### 4. Add filtered total counting

Add:

```rust
pub fn count_books_query(&self, query: &BookQuery) -> CoreResult<usize>;
```

It must return the count of **all books matching the structured filters**, ignoring:

- `limit`
- `offset`
- sort field
- sort direction

Use `COUNT(DISTINCT b.id)` because relation joins can duplicate book rows.

The count query must use the same filter/join semantics as `search_books_query`. Prefer a small private helper/refactor so the two paths do not maintain separate copies of author/tag/series/publisher/language/identifier join and condition construction.

Do not change filter matching semantics in this task.

### 5. Lift sort/pagination into library-domain types

In `crates/library/src/query.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibrarySortField {
    Id,
    Title,
    Format,
}
```

Extend `LibraryQuery` with:

```rust
pub offset: Option<usize>,
pub sort: LibrarySortField,
pub descending: bool,
```

Default behavior must remain ID ascending with no offset.

Add builders:

```rust
pub fn with_offset(self, value: usize) -> Self;
pub fn with_sort(self, field: LibrarySortField) -> Self;
pub fn descending(self) -> Self;
```

Map these values internally to the corresponding DB query values. Do not expose `BookSortField` in the library public API.

### 6. Add a library query-page result

In `crates/library/src/query.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryQueryPage {
    pub books: Vec<crate::catalog::LibraryBook>,
    pub total: usize,
    pub offset: usize,
    pub limit: Option<usize>,
}
```

If referencing `LibraryBook` through the crate path causes an awkward module cycle, import it normally; do not move `LibraryBook` into the query module.

Extend `LibraryCatalog` with:

```rust
pub fn query_page(&self, query: &LibraryQuery) -> CoreResult<LibraryQueryPage>;
```

`query_page` must:

1. execute the filtered/sorted/paginated DB query;
2. execute the filtered DB count;
3. map rows to `LibraryBook`;
4. return `offset` as `query.offset.unwrap_or(0)`;
5. return the requested `limit` unchanged.

Keep `query_books` working. It should naturally respect the new sort/offset fields because both methods use the same DB query mapping.

## Tests

Add focused tests at both DB and library layers.

### DB tests

Extend `crates/db/tests/query.rs` (or add one focused query-pagination test file) to prove at minimum:

1. title ascending sorting is case-insensitive and deterministic;
2. title descending sorting works;
3. `limit + offset` returns the expected subset;
4. offset without limit returns all remaining rows;
5. `count_books_query` returns the full filtered count even when the query contains limit/offset;
6. a relation filter such as author or tag still counts distinct books correctly.

### Library tests

Extend the existing library query tests to prove at minimum:

1. `LibrarySortField::Title` maps through and changes order;
2. offset + limit are honored by `query_page`;
3. `LibraryQueryPage.total` is the full filtered total, not page length;
4. `query_page.offset` and `query_page.limit` report the requested page metadata;
5. DB sort types are not part of the public library result types.

Keep tests compact and deterministic.

## Explicit non-goals

Do **not**:

- add author/series/rating/date sort fields yet;
- expand `LibraryBook` fields;
- add richer book-summary metadata;
- add GUI changes;
- add HTTP/JSON endpoints;
- change OPDS behavior;
- change facet semantics;
- add source abstractions;
- add async APIs;
- add caching;
- change schema or migrations;
- alter FTS/simple-search behavior;
- refactor unrelated DB/library code;
- clean unrelated warnings.

Richer visual-library summaries and GUI wiring are follow-up tasks.

## Files expected to change

Expected:

- `crates/db/src/query/mod.rs`
- `crates/db/src/database.rs`
- `crates/db/tests/query.rs` or a focused additional DB query test
- `crates/library/src/query.rs`
- `crates/library/src/catalog.rs`
- focused library tests if needed
- `docs/work/reports/0008.md`
- move this task from `docs/work/ready/` to `docs/work/done/`

No dependency or lockfile changes should be necessary.

If the task appears to require schema changes, GUI/server changes, a new dependency, or in-memory pagination of the complete result set, STOP and report the blocker instead of broadening scope.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-db --test query
cargo test -p caliberate-library
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass. Existing unrelated GUI warnings may remain.

## Handoff

Write `docs/work/reports/0008.md` with:

- summary;
- files changed;
- validation actually run and results;
- risks/unverified behavior;
- deviations/blockers.

Move this task to:

- `docs/work/done/0008-library-query-pages.md`

Commit and push exactly one bounded implementation branch:

- `codex/0008-library-query-pages`

Do not work on any other task.
