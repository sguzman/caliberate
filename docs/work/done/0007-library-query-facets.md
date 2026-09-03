# 0007 — Library-domain structured query and facets

## Goal

Lift the existing database structured-query and category-count capabilities into the public library domain so GUI/API consumers do not need to import `caliberate_db::query::BookQuery` or `caliberate_db::database::CategoryCount`.

This task is an adapter/domain-boundary task. Reuse existing database behavior. Do not add new SQL.

## Scope

### 1. Add a dedicated library query module

Create:

- `crates/library/src/query.rs`

Export it from `crates/library/src/lib.rs` with:

```rust
pub mod query;
```

Do not put these new domain types into `catalog.rs`; keep the module boundary explicit.

### 2. Add `LibraryQuery`

In `crates/library/src/query.rs`, add a public, library-domain query DTO:

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryQuery {
    pub title: Option<String>,
    pub author: Option<String>,
    pub tag: Option<String>,
    pub series: Option<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub identifier: Option<String>,
    pub format: Option<String>,
    pub limit: Option<usize>,
}
```

Provide:

```rust
impl LibraryQuery {
    pub fn new() -> Self;
    pub fn with_title(self, value: &str) -> Self;
    pub fn with_author(self, value: &str) -> Self;
    pub fn with_tag(self, value: &str) -> Self;
    pub fn with_series(self, value: &str) -> Self;
    pub fn with_publisher(self, value: &str) -> Self;
    pub fn with_language(self, value: &str) -> Self;
    pub fn with_identifier(self, value: &str) -> Self;
    pub fn with_format(self, value: &str) -> Self;
    pub fn with_limit(self, value: usize) -> Self;
}
```

The library-domain type must not expose `caliberate_db::query::BookQuery` in its public API.

Add an internal conversion from `LibraryQuery` (or `&LibraryQuery`) to the existing DB `BookQuery`. Do not duplicate search/filter logic in the library crate.

### 3. Add library facet domain types

In `crates/library/src/query.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryFacetKind {
    Authors,
    Tags,
    Series,
    Publishers,
    Ratings,
    Languages,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFacetValue {
    pub id: i64,
    pub name: String,
    pub count: i64,
}
```

Do not expose DB `CategoryCount` values from the library API.

### 4. Extend `LibraryCatalog`

In `crates/library/src/catalog.rs`, add these public read methods:

```rust
pub fn query_books(&self, query: &LibraryQuery) -> CoreResult<Vec<LibraryBook>>;

pub fn list_facets(
    &self,
    kind: LibraryFacetKind,
) -> CoreResult<Vec<LibraryFacetValue>>;
```

`query_books` must:

1. convert the library query into the existing DB `BookQuery`;
2. call `Database::search_books_query`;
3. map `BookRecord` values to `LibraryBook` using the existing library mapping.

`list_facets` must delegate exactly as follows:

- `Authors` -> `Database::list_author_categories`
- `Tags` -> `Database::list_tag_categories`
- `Series` -> `Database::list_series_categories`
- `Publishers` -> `Database::list_publisher_categories`
- `Ratings` -> `Database::list_rating_categories`
- `Languages` -> `Database::list_language_categories`

Map each DB `CategoryCount` to `LibraryFacetValue`.

Do not add SQL to `crates/library`.

## Tests

Add focused tests using a temporary database.

At minimum prove:

1. `LibraryQuery` filtering delegates correctly for a simple field such as title or format.
2. Author filtering works through the library-domain query. Seed authors with the existing `Database::add_book_authors` API.
3. Tag filtering works through the library-domain query. Seed tags with the existing `Database::add_book_tags` API.
4. `list_facets(LibraryFacetKind::Authors)` returns library-domain values with correct names/counts.
5. `list_facets(LibraryFacetKind::Tags)` returns library-domain values with correct names/counts.
6. `limit` is preserved through the library query mapping.

Tests may live in `query.rs`, `catalog.rs`, or a focused integration test file. Keep them compact.

## Explicit non-goals

Do **not**:

- add sorting;
- add offset/pagination;
- add total-count queries;
- add new SQL;
- change DB query semantics;
- change DB schema or migrations;
- change OPDS/server code;
- change GUI code;
- change HTTP/JSON APIs;
- add async APIs;
- introduce traits/repositories/source abstractions;
- expand `LibraryBook` fields;
- refactor unrelated library ingest/storage code;
- clean up unrelated warnings.

Sorting/pagination and richer visual-library summaries are separate follow-up tasks.

## Files expected to change

Expected:

- `crates/library/src/query.rs` (new)
- `crates/library/src/catalog.rs`
- `crates/library/src/lib.rs`
- tests if placed separately
- `docs/work/reports/0007.md`
- move this task from `docs/work/ready/` to `docs/work/done/`

No dependency change should be necessary because `caliberate-library` already depends on `caliberate-db`.

If implementing this task appears to require DB SQL/schema changes, server/GUI changes, or a new dependency, STOP and report the blocker instead of broadening scope.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-library
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass. Existing unrelated GUI warnings may remain.

## Handoff

Write `docs/work/reports/0007.md` with:

- summary;
- files changed;
- validation actually run and results;
- risks/unverified behavior;
- deviations/blockers.

Move this task to:

- `docs/work/done/0007-library-query-facets.md`

Commit and push exactly one bounded implementation branch:

- `codex/0007-library-query-facets`

Do not work on any other task.