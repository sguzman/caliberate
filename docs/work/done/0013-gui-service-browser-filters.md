# 0013 — Route visible GUI browser filters through the library service

## Goal

Move the visible GUI category-browser Include/Exclude filter execution off the local `BookRow` scan and onto the structured `LibraryQuery::metadata_filters` service path added by task `0012`.

This task is a **GUI/service integration step**, not real pagination yet.

After this task:

- Authors/Tags/Series/Publishers/Ratings/Languages browser filters must be translated to typed library-domain metadata filters;
- summary rows loaded for the GUI must already satisfy those browser filters;
- `LibraryView::apply_filters()` must no longer re-evaluate `browser_filters` against formatted `BookRow` strings;
- changing a browser filter must trigger a service-backed refresh.

The GUI may still keep the complete **service-filtered** working set in memory for residual local behavior. True page-at-a-time GUI browsing is a later task.

## Why now

Task `0011` added service sort parity.

Task `0012` added typed ANDed Include/Exclude metadata predicates with literal case-insensitive substring semantics and exact numeric Rating semantics.

The current visible GUI still does this locally in `LibraryView::apply_filters()`:

```rust
let browser_matches = self.browser_filters.iter().all(|filter| {
    ...
});
```

and `refresh_books()` still begins by loading every summary row using an ID-only `LibraryQuery`.

That means the visible browser filters still depend on the GUI's full in-memory row set.

## Current browser filter model

Keep the existing GUI types and persistence model:

```rust
struct BrowserFilter {
    category: BrowserCategory,
    value: String,
    mode: BrowserFilterMode,
}
```

Categories:

- Authors
- Tags
- Series
- Publishers
- Ratings
- Languages

Modes:

- Include
- Exclude

Do not replace these GUI presentation/persistence types with library-domain types.

## Scope

### 1. Add one explicit GUI -> library query translation seam

Add a small helper that maps one `BrowserFilter` to the corresponding library-domain metadata predicate.

Exact field mapping:

```text
BrowserCategory::Authors     -> LibraryMetadataFilterField::Authors
BrowserCategory::Tags        -> LibraryMetadataFilterField::Tags
BrowserCategory::Series      -> LibraryMetadataFilterField::Series
BrowserCategory::Publishers  -> LibraryMetadataFilterField::Publishers
BrowserCategory::Ratings     -> LibraryMetadataFilterField::Ratings
BrowserCategory::Languages   -> LibraryMetadataFilterField::Languages
```

Exact mode mapping:

```text
BrowserFilterMode::Include -> LibraryMetadataFilterMode::Include
BrowserFilterMode::Exclude -> LibraryMetadataFilterMode::Exclude
```

Preserve the filter value exactly. Do not lowercase, parse, or reinterpret it in the GUI.

The library/DB layer owns matching semantics.

### 2. Make the summary loader accept a base LibraryQuery

The current helper:

```rust
load_summary_rows(db, chunk_size)
```

always creates:

```rust
LibraryQuery::new()
    .with_sort(LibrarySortField::Id)
    .with_limit(chunk_size)
    .with_offset(offset)
```

Change the helper so the caller can supply the service query semantics while the loader owns bounded chunk pagination.

A shape such as:

```rust
load_summary_rows(db, base_query: &LibraryQuery, chunk_size)
```

is preferred.

For every chunk:

- preserve the caller's filters/sort/direction;
- replace/set only `limit` and `offset` for that chunk;
- stop when the page is empty or accumulated offset reaches `page.total`;
- do not duplicate filter construction inside the loader.

`LibraryQuery` is cloneable; use that rather than exposing DB types.

### 3. Build the visible refresh query from browser filters

In `refresh_books()`, construct a base `LibraryQuery` containing every current `self.browser_filters` entry through the translation seam.

For this task, keep the service base sort as deterministic ID ascending unless an existing helper cleanly maps the current visible sort without broadening scope.

Then call the bounded summary loader with that base query.

Result:

```text
self.all_books
```

becomes the complete current **service-browser-filtered** working set, not the complete unfiltered library.

This is intentional and is the bridge toward true pagination.

### 4. Remove local browser-filter execution from apply_filters

`LibraryView::apply_filters()` must stop iterating over `self.browser_filters`.

Keep residual local behavior unchanged in this task:

- exact `format_filter` behavior;
- `news_only_filter`;
- existing local sorting/grouping/secondary-sort behavior.

Do not migrate those here.

The local residual filter should therefore operate only on the rows already returned by the service browser-filter query.

### 5. Browser-filter mutations must request a service refresh

Every user-visible path that changes `self.browser_filters` must ensure `self.needs_refresh = true` so the service query is rerun.

Audit at least:

- cycling Include -> Exclude -> removed;
- removing a filter chip;
- Browser "Clear filter";
- clear all;
- stats drilldown;
- changing/clearing virtual library where stored browser filters are restored;
- any other direct mutation of `browser_filters` found in the file.

Do not perform DB queries inside the click handler itself if the existing `needs_refresh` lifecycle can handle it on the normal update path.

Persistence behavior for virtual-library filters must remain unchanged.

### 6. Preserve existing search behavior in this task

Do not broaden this task into global/scoped-search migration.

Current behavior may remain:

- SearchScope::All using `LibraryCatalog::search_books` candidate IDs;
- scoped Title/Authors/Tags/Series search filtering locally.

The important ordering is:

1. service query loads rows satisfying browser metadata filters;
2. existing search narrowing is applied as it is today;
3. residual format/news filters and local presentation sorting remain as they are today.

Do not make search results less restrictive by accidentally reloading an unfiltered set later.

### 7. Preserve category facet population

`refresh_browser()` may continue calling the existing global `list_facets` service path.

Do not make facet counts filter-sensitive in this task.

That is separate semantics and not required for moving row filtering to the service.

### 8. Status/working-set wording must not lie

Because `all_books` becomes the service-browser-filtered set, audit visible status text that assumes `all_books.len()` is the total unfiltered library.

Do not add a large new state system.

If an existing label such as:

```text
Filtered from N
```

would become factually misleading after this change, adjust/suppress that specific label conservatively.

Do not invent a fake total. The service page total is available during loading if a tiny return-struct/helper adjustment can expose it cleanly, but adding full pagination state is out of scope.

## Tests

Add focused GUI tests around pure/queryable seams. Do not require interactive desktop automation.

At minimum prove:

1. every BrowserCategory maps to the exact LibraryMetadataFilterField;
2. Include and Exclude map exactly;
3. multiple BrowserFilters become multiple metadata filters in the same order/value;
4. the bounded summary loader preserves a supplied metadata-filter query across multiple chunks;
5. a service-backed browser filter returns only matching summary rows;
6. removing/clearing the service filter and reloading restores previously excluded rows;
7. residual local format/news behavior still works on an already service-filtered set;
8. no local `browser_filters.iter().all(...)`-style row predicate remains in `apply_filters()`;
9. existing summary-loader chunk test is updated for the new helper signature and still proves multi-chunk collection.

If direct `LibraryView` mutation tests are practical with existing constructors, add a focused test proving a browser-filter change sets `needs_refresh`. Otherwise keep mutation logic centralized in a tiny helper and test that helper where practical.

## Explicit non-goals

Do **not**:

- add page controls;
- change the GUI to hold only one page yet;
- remove `all_books` yet;
- migrate format filtering;
- migrate News-only filtering;
- migrate scoped search;
- change global FTS/search semantics;
- make facets query-sensitive;
- change task 0012 filter semantics;
- change task 0011 sort semantics;
- change virtual-library persistence format;
- add schema/index/dependency changes;
- refactor the large GUI file beyond the narrow seams needed here;
- clean unrelated warnings.

## Expected files

Primarily:

- `crates/gui/src/views.rs`
- focused GUI tests in the existing test module/file
- `docs/work/reports/0013.md`
- move this task to `docs/work/done/0013-gui-service-browser-filters.md`

Library/DB crates should not require semantic changes. If a tiny export/import adjustment is truly required, keep it bounded and document it.

No dependency or lockfile changes should be necessary.

## Validation

Run on native Windows:

```text
cargo fmt --check
cargo test -p caliberate-gui
cargo test -p caliberate-library
cargo test -p caliberate-db
cargo check --workspace --locked
cargo test --workspace --locked
```

All must pass. Existing unrelated GUI warnings may remain.

## Handoff

Write `docs/work/reports/0013.md` with:

- summary;
- exact GUI -> service filter mapping;
- which local filters remain intentionally local;
- files changed;
- validation actually run and results;
- runtime behavior not interactively verified;
- deviations/blockers.

Move this task to:

- `docs/work/done/0013-gui-service-browser-filters.md`

Commit and push exactly one bounded implementation branch:

- `codex/0013-gui-service-browser-filters`

Do not work on any other task.
