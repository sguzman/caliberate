# 0011 — Extend library query sorting for the visible Calibre-like browser

## Goal

Extend the existing structured library query layer so it can deterministically sort by the metadata fields already exposed by the visible GUI browser.

This is a prerequisite for **real service-backed GUI pagination**. Task `0010` moved the GUI's visible book rows onto `LibraryCatalog::query_summary_page`, but the GUI still performs most sorting locally because `LibrarySortField` currently supports only ID, title, and format.

Do not change the GUI in this task. Add the missing reusable sort semantics at the DB/library service boundary and test them thoroughly.

## Existing baseline

`crates/db/src/query/mod.rs` currently defines:

```rust
pub enum BookSortField {
    Id,
    Title,
    Format,
}
```

`crates/library/src/query.rs` mirrors this with:

```rust
pub enum LibrarySortField {
    Id,
    Title,
    Format,
}
```

`Database::search_books_query` maps those enum values to hard-coded SQL expressions and appends `b.id ASC` as a deterministic tie-breaker for non-ID sorts.

`Database::count_books_query` shares the same filter construction but does not depend on sort expressions.

The current GUI exposes these sort modes:

- Title
- Authors
- Series
- Tags
- Formats
- Rating
- Publisher
- Languages
- Added
- Modified
- Pubdate
- ID

## Scope

### 1. Extend DB sort enum

In `crates/db/src/query/mod.rs`, extend `BookSortField` to:

```rust
pub enum BookSortField {
    Id,
    Title,
    Authors,
    Series,
    Tags,
    Format,
    Rating,
    Publisher,
    Languages,
    DateAdded,
    DateModified,
    PubDate,
}
```

Keep `Id` as the default.

Do not add free-form SQL sort strings.

### 2. Extend library-domain sort enum

In `crates/library/src/query.rs`, mirror the same semantic fields:

```rust
pub enum LibrarySortField {
    Id,
    Title,
    Authors,
    Series,
    Tags,
    Format,
    Rating,
    Publisher,
    Languages,
    DateAdded,
    DateModified,
    PubDate,
}
```

Map each variant explicitly in `LibraryQuery::to_db_query`.

Do not expose DB enums through the library-domain API.

### 3. Add deterministic SQL sort semantics

Extend `Database::search_books_query` using only hard-coded enum-selected SQL expressions/subqueries. Caller-controlled strings must never become SQL identifiers or sort fragments.

Required primary semantics:

- `Id` -> `b.id`
- `Title` -> `b.title COLLATE NOCASE`
- `Format` -> `b.format COLLATE NOCASE`
- `DateAdded` -> `COALESCE(b.timestamp, '')`
- `DateModified` -> `COALESCE(b.last_modified, '')`
- `PubDate` -> `COALESCE(b.pubdate, '')`
- `Authors` -> first author alphabetically, case-insensitive; books with no author use empty string
- `Tags` -> first tag alphabetically, case-insensitive; books with no tag use empty string
- `Series` -> series name case-insensitively; books with no series use empty string
- `Publisher` -> publisher name case-insensitively; books with no publisher use empty string
- `Rating` -> numeric rating; books with no rating use `0`
- `Languages` -> the first language according to `books_languages_link.item_order`, then link row ID; books with no language use empty string

Use correlated scalar subqueries or another compact approach that does **not** multiply the outer book rows. Do not add broad relation joins solely for sorting if that creates row multiplication or changes filtering semantics.

For multi-valued Authors/Tags, using the alphabetically first value is the intended service sort key. Do not concatenate every relation value into a giant SQL presentation string.

For Series, after the series-name primary key, add `b.series_index` using the same requested direction before the final ID tie-breaker.

### 4. Preserve deterministic tie behavior

For every non-ID sort, retain `b.id ASC` as the final deterministic tie-breaker.

For ascending/descending:

- the requested direction applies to the semantic primary sort key;
- for Series it also applies to `b.series_index`;
- the final `b.id` tie-breaker remains ascending, matching the existing query convention.

Do not use unstable implicit SQLite row ordering.

### 5. Keep summary/page behavior automatically compatible

`Database::search_book_summaries_query`, `Database::count_books_query`, `LibraryCatalog::query_page`, and `LibraryCatalog::query_summary_page` should continue working through the existing query conversion path.

Do not duplicate sorting logic in the summary loader or catalog layer.

## Tests

Add focused DB and library tests.

At minimum prove:

1. each newly added `BookSortField`/`LibrarySortField` maps and executes successfully;
2. Authors sorts by first alphabetical author, not insertion order;
3. Tags sorts by first alphabetical tag;
4. Series sorts by series name and then numeric `series_index`;
5. Rating sorts numerically and treats missing rating as zero;
6. Publisher sorts case-insensitively and handles missing publisher;
7. Languages uses `item_order` for the first-language sort key;
8. DateAdded, DateModified, and PubDate use the corresponding book columns;
9. descending reverses the semantic primary order;
10. equal semantic keys use ascending book ID as the deterministic final tie-breaker;
11. `query_summary_page` preserves the order produced by the structured query.

Keep fixtures compact. Reuse existing DB mutation helpers.

## Explicit non-goals

Do **not**:

- change the GUI or `SortMode` yet;
- add GUI pagination controls;
- add browser include/exclude query semantics yet;
- change FTS/global-search behavior;
- change filter semantics;
- change facet semantics;
- change summary fields;
- add schema or migrations;
- add indexes in this task;
- add arbitrary/free-form SQL sorting;
- add secondary-sort arrays to `LibraryQuery`;
- change OPDS/server/HTTP behavior;
- add a dependency;
- refactor unrelated DB or GUI code;
- clean unrelated warnings.

A subsequent task will extend structured filter semantics required by the GUI; after that the visible browser can move to true service-backed pagination.

## Files expected to change

Expected:

- `crates/db/src/query/mod.rs`
- `crates/db/src/database.rs`
- focused DB query tests
- `crates/library/src/query.rs`
- focused library query tests
- `docs/work/reports/0011.md`
- move this task from `docs/work/ready/` to `docs/work/done/`

No dependency or lockfile changes should be necessary.

If implementing one of these sorts appears to require schema changes or a new dependency, STOP and report the blocker rather than broadening scope.

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

Write `docs/work/reports/0011.md` with:

- summary;
- exact sort semantics implemented;
- files changed;
- validation actually run and results;
- risks/unverified behavior;
- deviations/blockers.

Move this task to:

- `docs/work/done/0011-library-query-sort-parity.md`

Commit and push exactly one bounded implementation branch:

- `codex/0011-library-query-sort-parity`

Do not work on any other task.
