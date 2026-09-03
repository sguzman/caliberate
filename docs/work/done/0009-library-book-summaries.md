# 0009 — Rich library book-summary read model

## Goal

Add an efficient, library-domain book-summary read model that contains the metadata already required by the existing Calibre-like central table/grid, without making GUI/API consumers perform per-book database lookups.

This is the last read-model seam before beginning visible GUI migration. The result must be suitable for one paged library query and must not introduce N+1 database calls.

## Existing baseline

Task `0008-library-query-pages` provides:

- structured library filters;
- deterministic ID/title/format sorting;
- database-backed limit/offset pagination;
- filtered total counts;
- `LibraryCatalog::query_page` returning basic `LibraryBook` rows.

The current GUI `BookRow` already displays or stores:

- ID;
- title;
- format;
- path;
- authors;
- tags;
- series;
- rating;
- publisher;
- languages;
- cover presence;
- added date;
- modified date;
- publication date.

Today the GUI is coupled directly to database record/category types. We need a reusable structured read model before migrating that GUI path.

## Scope

### 1. Add a database summary record

In `crates/db/src/database.rs`, add a read-only database DTO:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BookSummaryRecord {
    pub id: i64,
    pub title: String,
    pub format: String,
    pub path: String,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub series: Option<SeriesEntry>,
    pub rating: Option<i64>,
    pub publisher: Option<String>,
    pub languages: Vec<String>,
    pub has_cover: bool,
    pub timestamp: Option<String>,
    pub last_modified: Option<String>,
    pub pubdate: Option<String>,
}
```

Use the existing `SeriesEntry` type for the DB-internal/public DB DTO. Do not change existing `BookRecord` or `BookExtras` fields.

### 2. Add a paged database summary query

Add:

```rust
pub fn search_book_summaries_query(
    &self,
    query: &BookQuery,
) -> CoreResult<Vec<BookSummaryRecord>>;
```

Required behavior:

1. Use the existing `search_books_query(query)` to obtain the filtered/sorted/paginated base page. Do not duplicate the structured filter/order/pagination builder.
2. If the page is empty, return an empty vector immediately.
3. Load the additional metadata for all page book IDs using **batched queries whose count is independent of page length**.
4. Preserve the exact order returned by `search_books_query`.
5. Do not call existing per-book getters inside a loop.

At minimum batch-load:

- book extras needed here: `has_cover`, `timestamp`, `last_modified`, `pubdate`;
- authors;
- tags;
- series name + `books.series_index`;
- publisher;
- rating;
- languages.

It is acceptable to use several small SQL queries (for example one per relation family) as long as the number of queries is fixed for a page and does not grow with the number of books.

For batched ID restrictions:

- generate SQL placeholder text only from the number of IDs;
- bind every ID as a SQL parameter;
- do not interpolate caller-controlled values into SQL.

Existing relation ordering should be deterministic. Preserve meaningful existing order where the schema has one (for example language `item_order`); otherwise use a stable ordering such as relation row ID/name.

Do not change schema or migrations.

### 3. Add library-domain summary types in a dedicated module

Create:

- `crates/library/src/summary.rs`

Export it from `crates/library/src/lib.rs`:

```rust
pub mod summary;
```

Add:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct LibrarySeriesSummary {
    pub name: String,
    pub index: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LibraryBookSummary {
    pub id: i64,
    pub title: String,
    pub format: String,
    pub path: String,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub series: Option<LibrarySeriesSummary>,
    pub rating: Option<i64>,
    pub publisher: Option<String>,
    pub languages: Vec<String>,
    pub has_cover: bool,
    pub date_added: Option<String>,
    pub date_modified: Option<String>,
    pub pubdate: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LibrarySummaryPage {
    pub books: Vec<LibraryBookSummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: Option<usize>,
}
```

Mapping semantics:

- DB `timestamp` -> library `date_added`;
- DB `last_modified` -> library `date_modified`;
- DB `pubdate` -> library `pubdate`;
- DB `SeriesEntry` -> `LibrarySeriesSummary`.

Do not expose `BookSummaryRecord`, `SeriesEntry`, or any other `caliberate-db` type through the library public result types.

### 4. Extend `LibraryCatalog`

Add:

```rust
pub fn query_summary_page(
    &self,
    query: &LibraryQuery,
) -> CoreResult<LibrarySummaryPage>;
```

It must:

1. convert the `LibraryQuery` to the existing DB `BookQuery`;
2. call `Database::search_book_summaries_query` for the paged rich rows;
3. call `Database::count_books_query` for the full filtered total;
4. map database summary records into library-domain summary records;
5. return `offset` as `query.offset.unwrap_or(0)`;
6. return `limit` unchanged.

Keep `query_books` and `query_page` working unchanged.

### 5. No N+1 metadata resolution

This is an explicit acceptance constraint.

The implementation must **not** do this inside a loop over the page:

- `list_book_authors(book_id)`;
- `list_book_tags(book_id)`;
- `get_book_series(book_id)`;
- `get_book_extras(book_id)`;
- or equivalent one-query-per-book calls.

Use page-wide batched relation queries.

## Tests

### DB tests

Add focused tests proving at minimum:

1. a summary query returns authors, tags, series name/index, publisher, rating, languages, cover flag, timestamp, last-modified, and pubdate correctly;
2. summary rows preserve the sort/order of `search_books_query`;
3. limit + offset restrict the summary page correctly;
4. an empty page returns an empty summary vector cleanly;
5. multiple books with shared/multiple relation values do not duplicate summary rows.

Use existing database mutation helpers to seed metadata wherever possible. Do not add production-only setters solely for tests.

### Library tests

Add focused tests proving at minimum:

1. `query_summary_page` maps the rich DB record into library-domain fields correctly;
2. `date_added`, `date_modified`, and `pubdate` use the required mappings;
3. `LibrarySeriesSummary` contains name and index;
4. `total` is the full filtered count rather than the page length;
5. page order/offset/limit are preserved;
6. library result types do not contain DB DTO types.

Keep tests compact.

## Explicit non-goals

Do **not**:

- change the GUI yet;
- change server/OPDS/HTTP behavior;
- add HTTP/JSON endpoints;
- add new sort fields in this task;
- add cover image loading/decoding/caching;
- add book-detail/comment/note/identifier payloads to the summary;
- change `LibraryBook`;
- change structured filter semantics;
- change facet semantics;
- add async APIs;
- add caches;
- add source abstractions;
- change schema or migrations;
- add a new dependency;
- refactor unrelated DB/library code;
- clean unrelated GUI warnings.

The next task after this one will begin migrating the visible Calibre-like GUI browse/search/category path onto the library service.

## Files expected to change

Expected:

- `crates/db/src/database.rs`
- focused DB tests, preferably `crates/db/tests/query.rs` or a new focused summary test file
- `crates/library/src/summary.rs` (new)
- `crates/library/src/lib.rs`
- `crates/library/src/catalog.rs`
- focused library tests
- `docs/work/reports/0009.md`
- move this task from `docs/work/ready/` to `docs/work/done/`

No dependency or lockfile changes should be necessary.

If efficient page-wide metadata loading appears to require a schema change or new dependency, STOP and report the blocker rather than falling back to N+1 queries.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-db
cargo test -p caliberate-library
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass. Existing unrelated GUI warnings may remain.

## Handoff

Write `docs/work/reports/0009.md` with:

- summary;
- files changed;
- validation actually run and results;
- confirmation that metadata loading is batched rather than per-book;
- risks/unverified behavior;
- deviations/blockers.

Move this task to:

- `docs/work/done/0009-library-book-summaries.md`

Commit and push exactly one bounded implementation branch:

- `codex/0009-library-book-summaries`

Do not work on any other task.
