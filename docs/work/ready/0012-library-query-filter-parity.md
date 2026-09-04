# 0012 — Add structured include/exclude metadata filters to library queries

## Goal

Extend the reusable DB/library query layer with the positive/negative metadata-filter semantics currently implemented locally by the GUI category browser.

This is the next prerequisite for real service-backed GUI pagination.

Task `0011` completed sort parity. The visible GUI still keeps the full `all_books` set because its category-browser include/exclude filters are evaluated locally in `LibraryView::apply_filters()`.

Do **not** change the GUI in this task. Add the reusable query semantics first and test them thoroughly.

## Current GUI semantics to preserve

The GUI stores category filters as:

```rust
struct BrowserFilter {
    category: BrowserCategory,
    value: String,
    mode: BrowserFilterMode,
}

enum BrowserFilterMode {
    Include,
    Exclude,
}
```

Supported browser categories:

- Authors
- Tags
- Series
- Publishers
- Ratings
- Languages

Current local behavior is:

```text
all active filters are ANDed together

Include -> the book must match that filter
Exclude -> the book must not match that filter
```

This applies even when multiple filters use the same category.

Examples:

- include tag `history` AND include tag `military` => a book must match both;
- include author `smith` AND exclude tag `draft` => both predicates must hold;
- exclude author `smith` allows books with no authors, because they do not match Smith.

Current string matching is case-insensitive substring matching for Authors, Tags, Series, Publishers, and Languages.

Ratings use exact textual/numeric equality.

The service layer should preserve these visible semantics closely enough that later GUI migration does not change results.

## Existing baseline

`BookQuery` and `LibraryQuery` already support legacy single positive fields:

- author
- tag
- series
- publisher
- language
- plus title/identifier/format

Keep those existing fields working for compatibility.

Do not silently reinterpret them as the new filter collection.

## Scope

### 1. Add DB-domain structured metadata filters

In the DB query module, add bounded typed query structures equivalent to:

```rust
pub enum BookMetadataFilterField {
    Authors,
    Tags,
    Series,
    Publishers,
    Ratings,
    Languages,
}

pub enum BookMetadataFilterMode {
    Include,
    Exclude,
}

pub struct BookMetadataFilter {
    pub field: BookMetadataFilterField,
    pub mode: BookMetadataFilterMode,
    pub value: String,
}
```

Names may differ slightly if repository style strongly prefers another spelling, but preserve the same explicit semantics.

Add:

```rust
pub metadata_filters: Vec<BookMetadataFilter>
```

to `BookQuery`, defaulting to empty.

Provide a small builder such as:

```rust
with_metadata_filter(...)
```

Do not add caller-provided SQL fragments.

### 2. Mirror filters in the library domain

Add library-domain equivalents:

```rust
LibraryMetadataFilterField
LibraryMetadataFilterMode
LibraryMetadataFilter
```

and:

```rust
pub metadata_filters: Vec<LibraryMetadataFilter>
```

to `LibraryQuery`.

Map every variant explicitly in `LibraryQuery::to_db_query`.

Do not expose DB query enums/types as the public library API.

### 3. Implement DB filtering with row-safe EXISTS / NOT EXISTS semantics

Extend the shared query construction so both result queries and count queries receive identical metadata predicates.

Prefer correlated `EXISTS` / `NOT EXISTS` subqueries rather than broad outer relation joins for these new filters.

Required semantics:

#### Authors

Include:

```text
EXISTS related author whose name case-insensitively contains value
```

Exclude:

```text
NOT EXISTS related author whose name case-insensitively contains value
```

#### Tags

Same semantics over tag names.

#### Series

Same semantics over the related series name.

#### Publishers

Same semantics over the related publisher name.

#### Languages

Same semantics over related language code/name value currently surfaced by the library summary/facet path.

#### Ratings

Include:

```text
book has a related numeric rating equal to requested rating
```

Exclude:

```text
book does not have that numeric rating
```

A book with no rating:

- fails an Include rating predicate;
- passes an Exclude rating predicate.

Do not make rating a substring comparison.

If the filter value cannot be parsed as the supported numeric rating representation, return a clear query/input error or another deterministic non-match behavior consistent with existing crate conventions. Do not interpolate it into SQL.

### 4. AND every active structured metadata filter

Every entry in `metadata_filters` must contribute a separate predicate joined with logical AND.

Do not collapse multiple same-category Includes into OR.

Examples that must work:

```text
Tag Include "history"
Tag Include "military"
```

requires both matches.

```text
Author Include "smith"
Tag Exclude "draft"
Language Include "en"
```

requires all three predicates.

### 5. Preserve legacy query fields and all task 0011 sorting

Existing fields such as `author`, `tag`, `series`, `publisher`, and `language` must continue to work.

If both a legacy field and a structured metadata filter are present, they are both active and therefore AND together.

Preserve:

- all sort fields from task `0011`;
- deterministic non-ID `b.id ASC` tie-breaking;
- limit/offset;
- filtered totals;
- summary-page ordering;
- existing facet behavior.

### 6. Count/result semantic parity

`Database::count_books_query` and the result query must share the same new metadata filter predicates.

A paginated library query must report the total number of books matching the full filter set, not merely the current page.

Do not duplicate independent filtering logic between count and search paths.

## Matching details

For Authors/Tags/Series/Publishers/Languages:

- use parameterized case-insensitive contains matching;
- no caller string may become a SQL identifier or SQL fragment;
- escape/bind values through the existing parameter system;
- missing relations fail Include and pass Exclude naturally.

Do not concatenate all related values merely to filter them.

## Tests

Add focused DB and library tests.

At minimum prove:

1. Include works for each of Authors, Tags, Series, Publishers, Ratings, Languages.
2. Exclude works for each category.
3. Matching for non-rating string categories is case-insensitive substring matching.
4. Two Includes in the same category are ANDed, not ORed.
5. Include + Exclude across categories are ANDed.
6. Missing relation values fail Include and pass Exclude.
7. Rating is numeric/exact rather than substring based.
8. Multiple active metadata filters do not duplicate outer book rows.
9. Legacy single positive filters still work.
10. Legacy field + structured filter are both applied.
11. `count_books_query` returns the same full-filter total used by paginated result queries.
12. `LibraryQuery` maps every structured field/mode explicitly and `query_summary_page` respects the filters while preserving requested sort order.

Use compact temporary DB fixtures and existing mutation helpers.

## Explicit non-goals

Do **not**:

- change `crates/gui` or `BrowserFilter`;
- migrate GUI filtering to the service yet;
- add GUI pagination controls;
- change global/FTS search behavior;
- change sort semantics from task `0011`;
- change facets;
- add schema migrations;
- add indexes;
- add dependencies;
- add arbitrary SQL predicates;
- add generic boolean-expression trees;
- implement OR groups;
- implement saved-search parsing;
- change virtual-library persistence;
- refactor unrelated DB code;
- clean unrelated warnings.

A subsequent task will translate the existing GUI browser filters into these library-domain predicates. Real GUI pagination comes after the visible filter path no longer depends on the full in-memory set.

## Expected files

Expected:

- `crates/db/src/query/mod.rs`
- `crates/db/src/database.rs`
- focused DB query tests
- `crates/library/src/query.rs`
- focused library query tests
- `docs/work/reports/0012.md`
- move this task to `docs/work/done/0012-library-query-filter-parity.md`

No dependency or lockfile changes should be necessary.

If correct implementation appears to require schema changes or a new dependency, STOP and report the blocker rather than broadening scope.

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

Write `docs/work/reports/0012.md` with:

- summary;
- exact Include/Exclude semantics;
- files changed;
- validation actually run and results;
- risks/unverified behavior;
- deviations/blockers.

Move this task to:

- `docs/work/done/0012-library-query-filter-parity.md`

Commit and push exactly one bounded implementation branch:

- `codex/0012-library-query-filter-parity`

Do not work on any other task.
